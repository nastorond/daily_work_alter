import { itemDepth, itemText } from "./markdown.js";

/**
 * Regrouping a week of daily logs by project.
 *
 * The log is written day by day, but the weekly report is read project by
 * project, and re-sorting a week by hand every Friday was the chore this
 * replaces. Projects are marked with markdown headings, inconsistently:
 *
 *   10/27   #### 태양광 / #### I3View        -> #### is the project
 *   11/04   ### 태양광 / #### 중간 학습       -> ### is the project, #### a
 *                                               section inside it
 *
 * So within each day, the largest heading used that day (fewest `#`) names a
 * project and any smaller heading is a section of the current project. Same
 * names are merged across the week, case-insensitively. Lines written before
 * the day's first heading go to "기타".
 */

export const MISC = "기타";

const HEADING = /^(#+)\s*(.*)$/;

function heading(line) {
  const m = HEADING.exec(itemText(line));
  return m ? { level: m[1].length, text: m[2].trim() } : null;
}

function keyOf(name) {
  return name.toLowerCase().replace(/\s+/g, " ").trim();
}

/** A bucket of lines with exact duplicates dropped (daily status lines repeat). */
function bucket() {
  return { lines: [], seen: new Set() };
}

function push(b, line) {
  const k = itemText(line);
  if (b.seen.has(k)) return;
  b.seen.add(k);
  b.lines.push(line);
}

/**
 * @param days  [{ date, items }] in date order
 * @returns     [{ name, intro: string[], sections: [{ name, lines: string[] }] }]
 *              in first-seen order, "기타" last, empty projects dropped
 */
export function groupByProject(days) {
  const projects = new Map();

  const project = (name) => {
    const k = keyOf(name);
    if (!projects.has(k)) {
      projects.set(k, { name, intro: bucket(), sections: new Map() });
    }
    return projects.get(k);
  };

  const section = (proj, name) => {
    const k = keyOf(name);
    if (!proj.sections.has(k)) proj.sections.set(k, { name, ...bucket() });
    return proj.sections.get(k);
  };

  for (const day of days) {
    const levels = day.items.map(heading).filter(Boolean).map((h) => h.level);
    const top = levels.length ? Math.min(...levels) : Infinity;

    let proj = null;
    let sec = null;
    for (const line of day.items) {
      const h = heading(line);
      if (h && h.text) {
        if (h.level <= top || !proj) {
          proj = project(h.text);
          sec = null;
        } else {
          sec = section(proj, h.text);
        }
        continue;
      }
      if (h) continue; // a bare "###" with no name
      push(sec ?? (proj ?? project(MISC)).intro, line);
    }
  }

  // A day that only uses `####` makes each of them a project, though on another
  // day the same name sat under `### 태양광` as a section. The week knows better
  // than the day: a "project" that is some other project's section is folded
  // into that section.
  const miscKey = keyOf(MISC);
  for (const [k, p] of [...projects]) {
    if (k === miscKey) continue;
    const owner = [...projects.values()].find((q) => q !== p && q.sections.has(k));
    if (!owner) continue;
    const target = owner.sections.get(k);
    for (const l of p.intro.lines) push(target, l);
    for (const s of p.sections.values()) for (const l of s.lines) push(target, l);
    projects.delete(k);
  }

  for (const p of projects.values()) foldIntoSections(p);

  const out = [];
  let misc = null;
  for (const p of projects.values()) {
    const sections = [...p.sections.values()]
      .filter((s) => s.lines.length > 0)
      .map((s) => ({ name: s.name, lines: s.lines }));
    const entry = { name: p.name, intro: p.intro.lines, sections };
    if (entry.intro.length === 0 && sections.length === 0) continue;
    if (keyOf(p.name) === keyOf(MISC)) misc = entry;
    else out.push(entry);
  }
  if (misc) out.push(misc);
  return out;
}

/**
 * The same topic is sometimes a plain line with indented lines under it on one
 * day (`- UM 빈 구간 백필` / `  - 일사량 92.8%`) and a section heading on another
 * (`#### UM 빈 구간 백필`). When a project's own top-level line names one of its
 * sections, its indented lines move into that section, so the topic appears
 * once. They go in front, since the section heading form tends to come later.
 */
function foldIntoSections(p) {
  const intro = p.intro.lines;
  const kept = [];
  for (let i = 0; i < intro.length; i++) {
    const line = intro[i];
    const sec = itemDepth(line) === 0 ? p.sections.get(keyOf(itemText(line))) : undefined;
    if (!sec) {
      kept.push(line);
      continue;
    }
    const moved = [];
    while (i + 1 < intro.length && itemDepth(intro[i + 1]) === 1) {
      moved.push(intro[++i].trimStart());
    }
    const fresh = moved.filter((l) => !sec.seen.has(itemText(l)));
    for (const l of fresh) sec.seen.add(itemText(l));
    sec.lines = [...fresh, ...sec.lines];
  }
  p.intro.lines = kept;
}

/**
 * The weekly-report layout (주간업무 일지): `1. 프로젝트` / `ㅇ 항목` / `- 세부`.
 * A project's own lines are items; a section becomes an item and its lines
 * become details. Indented lines go one step further in.
 */
export function toReport(project, number) {
  const out = [`${number}. ${project.name}`];
  for (const line of project.intro) {
    out.push(itemDepth(line) === 1 ? ` - ${itemText(line)}` : `ㅇ ${itemText(line)}`);
  }
  for (const s of project.sections) {
    out.push(`ㅇ ${s.name}`);
    for (const line of s.lines) {
      out.push(itemDepth(line) === 1 ? `   - ${itemText(line)}` : ` - ${itemText(line)}`);
    }
  }
  return out.join("\n");
}

/** The same grouping as markdown, lines kept as written. */
export function toMarkdown(project) {
  const out = [`### ${project.name}`, ...project.intro];
  for (const s of project.sections) {
    out.push(`#### ${s.name}`, ...s.lines);
  }
  return out.join("\n");
}

export function renderAll(projects, format) {
  return projects
    .map((p, i) => (format === "report" ? toReport(p, i + 1) : toMarkdown(p)))
    .join("\n\n");
}

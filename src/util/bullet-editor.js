import { CHILD_PREFIX } from "./markdown.js";

/**
 * Shared key handling for the bullet textareas — the daily input, the weekly
 * "today" input, and the weekly inline edit all behave identically:
 *
 *   Enter       new bullet on the next line, keeping the current indent
 *   Tab         indent one level (children only go one deep)
 *   Shift+Tab   outdent
 *   Ctrl+Enter  submit
 *   Escape      submit
 *
 * `onSubmit` differs per site (close the window vs. blur the inline editor), so
 * it's passed in rather than assumed.
 */
export function attachBulletEditor(textarea, { onChange, onSubmit } = {}) {
  const changed = () => onChange?.();
  const submit = () => onSubmit?.();

  textarea.addEventListener("focus", () => {
    if (textarea.value === "") {
      setValue(textarea, "- ", "- ".length);
    }
  });

  textarea.addEventListener("input", changed);

  textarea.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && e.ctrlKey) {
      e.preventDefault();
      submit();
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      submit();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      insertBullet(textarea);
      changed();
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      shiftIndent(textarea, e.shiftKey ? -1 : 1);
      changed();
    }
  });
}

/** Start/end offsets of the line containing `pos`. */
function lineStart(value, pos) {
  return value.lastIndexOf("\n", pos - 1) + 1;
}

function setValue(textarea, value, caret) {
  textarea.value = value;
  textarea.selectionStart = textarea.selectionEnd = caret;
}

/** Enter: open a new bullet at the same indent level as the current line. */
function insertBullet(textarea) {
  const { value, selectionStart: from, selectionEnd: to } = textarea;
  const start = lineStart(value, from);
  const leading = /^[ \t]*/.exec(value.slice(start))[0];
  const indent = leading.length >= CHILD_PREFIX.length ? CHILD_PREFIX : "";
  const insert = `\n${indent}- `;
  setValue(textarea, value.slice(0, from) + insert + value.slice(to), from + insert.length);
}

/** Tab / Shift+Tab: add or remove one level of indent on the current line. */
function shiftIndent(textarea, direction) {
  const { value, selectionStart: caret } = textarea;
  const start = lineStart(value, caret);
  const leading = /^[ \t]*/.exec(value.slice(start))[0];

  if (direction > 0) {
    // One level only — a line that is already a child stays put.
    if (leading.length >= CHILD_PREFIX.length) return;
    setValue(
      textarea,
      value.slice(0, start) + CHILD_PREFIX + value.slice(start),
      caret + CHILD_PREFIX.length
    );
    return;
  }

  if (leading.length === 0) return;
  const remove = Math.min(leading.length, CHILD_PREFIX.length);
  setValue(
    textarea,
    value.slice(0, start) + value.slice(start + remove),
    Math.max(start, caret - remove)
  );
}

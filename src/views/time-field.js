/**
 * A 24-hour 시:분 input.
 *
 * `<input type="time">` would be less code, but Chromium renders it in the
 * browser's UI locale and ignores the `lang` attribute — so on a Korean Windows
 * it shows "오후 05:00" while config.json, the log files and the rest of the app
 * all speak in 17:00. Two number boxes keep what you read and what gets stored
 * the same thing, whatever the OS locale is.
 *
 * Values go in and come out as "HH:MM" so the existing validation (which
 * compares the two times as zero-padded strings) keeps working unchanged.
 */

const pad = (n) => String(n).padStart(2, "0");

export function timeFieldHtml(id) {
  return `<span class="time-field" id="${id}">
      <input type="number" class="tf-hour" min="0" max="23" step="1" inputmode="numeric" aria-label="시" />
      <span class="tf-sep">:</span>
      <input type="number" class="tf-min" min="0" max="59" step="5" inputmode="numeric" aria-label="분" />
    </span>`;
}

function parts(root, id) {
  const el = root.querySelector(`#${id}`);
  return { hour: el.querySelector(".tf-hour"), min: el.querySelector(".tf-min") };
}

export function setTime(root, id, hhmm) {
  const { hour, min } = parts(root, id);
  const [h, m] = String(hhmm ?? "").split(":");
  hour.value = pad(clamp(Number(h), 0, 23));
  min.value = pad(clamp(Number(m), 0, 59));
}

/** "HH:MM", or null when a box is left empty or out of range. */
export function getTime(root, id) {
  const { hour, min } = parts(root, id);
  const h = readBox(hour, 23);
  const m = readBox(min, 59);
  if (h === null || m === null) return null;
  return `${pad(h)}:${pad(m)}`;
}

/** Pads on blur so the field settles to 09:00 rather than 9:0. */
export function attachTimeField(root, id) {
  const { hour, min } = parts(root, id);
  for (const [box, max] of [
    [hour, 23],
    [min, 59],
  ]) {
    box.addEventListener("blur", () => {
      const v = readBox(box, max);
      box.value = v === null ? "" : pad(v);
    });
  }
}

function readBox(box, max) {
  const raw = box.value.trim();
  if (raw === "") return null;
  const n = Number(raw);
  if (!Number.isInteger(n) || n < 0 || n > max) return null;
  return n;
}

function clamp(n, lo, hi) {
  if (!Number.isFinite(n)) return lo;
  return Math.min(hi, Math.max(lo, Math.trunc(n)));
}

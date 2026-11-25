import { inCaseOf } from "./suggestCase";

/** What can be finished from a result's name: applications, settings and commands. */
const COMPLETABLE = new Set(["app", "setting", "action"]);

/**
 * The typed search finished with the name of the best application, setting or
 * command that starts with it ("calc" → "calculator"), in the case being
 * typed. `null` when none does, or when the name is already all typed.
 */
export function suggestFromResults(query, sections) {
  const typed = query.replace(/^\s+/, "");
  if (typed.trim().length < 2) return null;
  const wanted = typed.toLowerCase();
  for (const section of sections ?? []) {
    for (const result of section.results) {
      if (!COMPLETABLE.has(result.kind)) continue;
      const title = result.title;
      if (title.toLowerCase().startsWith(wanted) && title.length > typed.length) {
        return query.slice(0, query.length - typed.length) + typed + inCaseOf(typed, title.slice(typed.length));
      }
    }
  }
  return null;
}

/** "Adobe Photoshop 2027" → ["adobe", "photoshop", "2027"] */
const wordsOf = (text) => text.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);

/**
 * The whole name of the best application, setting or command when the search
 * names it without starting it: from the middle ("photoshop" → "Adobe
 * Photoshop 2027") or out of order ("photoshop adobe"), every typed word being
 * the start of a word in the name. Shown after a dash, and put in the field
 * when accepted. `null` when the best match starts with the search (that is
 * {@link suggestFromResults}' job) or nothing matches.
 */
export function suggestFullName(query, sections) {
  const wanted = query.trim().toLowerCase();
  const typed = wordsOf(wanted);
  if (wanted.length < 2 || typed.length === 0) return null;
  for (const section of sections ?? []) {
    for (const result of section.results) {
      if (!COMPLETABLE.has(result.kind)) continue;
      // the best match is already being typed in order
      if (result.title.toLowerCase().startsWith(wanted)) return null;
      const words = wordsOf(result.title);
      if (typed.every((w) => words.some((word) => word.startsWith(w)))) return result.title;
    }
  }
  return null;
}

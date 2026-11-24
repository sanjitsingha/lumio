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

/** The rest of a word in the case being typed: "n" goes on as "ew york", "N" as "ew York", "LO" as "NDON". */
export function inCaseOf(typed, rest) {
  if (typed === typed.toLowerCase()) return rest.toLowerCase();
  if (typed.length >= 2 && typed === typed.toUpperCase()) return rest.toUpperCase();
  return rest;
}

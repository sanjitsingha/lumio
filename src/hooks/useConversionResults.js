import { useEffect, useMemo, useState } from "react";
import { api } from "../services/tauri";
import { relatedTerms, relevantResults } from "../utils/currency";

/** Waits for the typing to pause. */
const DELAY = 150;
/** Kept apart from `useSearch`'s request ids. */
let nextRequest = 1_000_000_000;

/**
 * What to list below a currency or measurement answer: for money, apps and
 * files named after these currencies; for both, a web search for the
 * conversion in clean words ("1 USD to INR", "5 ft to cm"), whatever was
 * typed. Results of another pair never show.
 */
export function useConversionResults(conversion) {
  const [local, setLocal] = useState({ pair: null, results: [] });
  const [web, setWeb] = useState({ pair: null, result: null });
  const from = conversion?.from;
  const to = conversion?.to;
  const isMoney = Boolean(conversion) && !conversion.measure;
  const pair = from ? `${from}|${to}` : null;
  const webQuery = conversion ? (conversion.webQuery ?? `${conversion.amount} ${from} to ${to}`) : null;

  useEffect(() => {
    if (!pair || !isMoney) return undefined;
    let alive = true;
    const timer = setTimeout(async () => {
      const searches = relatedTerms(from, to).map((term) => api.search(term, nextRequest++).catch(() => null));
      const responses = await Promise.all(searches);
      if (alive) setLocal({ pair, results: relevantResults(responses, from, to) });
    }, DELAY);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [pair, isMoney, from, to]);

  useEffect(() => {
    if (!pair) return undefined;
    let alive = true;
    const timer = setTimeout(async () => {
      const response = await api.search(webQuery, nextRequest++).catch(() => null);
      const result = response?.sections.flatMap((section) => section.results).find((r) => r.kind === "web") ?? null;
      if (alive) setWeb({ pair, result });
    }, DELAY);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [pair, webQuery]);

  return useMemo(() => {
    const results = isMoney && local.pair === pair ? local.results : [];
    return web.pair === pair && web.result ? [...results, web.result] : results;
  }, [local, web, pair, isMoney]);
}

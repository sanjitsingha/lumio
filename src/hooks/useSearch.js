import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../services/tauri";

/** Only show a spinner when a search is actually slow. */
const SPINNER_DELAY = 140;

/**
 * Debounced backend search. Responses for superseded queries are dropped by
 * request id, so fast typing never shows stale results.
 */
export function useSearch(delay = 60) {
  const [query, setQuery] = useState("");
  const [response, setResponse] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const requestRef = useRef(0);

  useEffect(() => {
    const id = ++requestRef.current;
    if (!query.trim()) {
      setResponse(null);
      setLoading(false);
      setError(null);
      return undefined;
    }
    let spinner;
    const timer = setTimeout(async () => {
      spinner = setTimeout(() => requestRef.current === id && setLoading(true), SPINNER_DELAY);
      try {
        const result = await api.search(query, id);
        if (requestRef.current === id) {
          setResponse(result);
          setError(null);
        }
      } catch (e) {
        if (requestRef.current === id) setError(e);
      } finally {
        clearTimeout(spinner);
        if (requestRef.current === id) setLoading(false);
      }
    }, delay);
    return () => {
      clearTimeout(timer);
      clearTimeout(spinner);
    };
  }, [query, delay]);

  const reset = useCallback(() => {
    requestRef.current++;
    setQuery("");
    setResponse(null);
    setLoading(false);
    setError(null);
  }, []);

  // The response only matches the input once it has caught up with it.
  const settled = response && response.query === query ? response : null;
  return { query, setQuery, response: settled, staleResponse: response, loading, error, reset };
}

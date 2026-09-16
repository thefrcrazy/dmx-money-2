import { useEffect, useState } from 'react';

export const localToday = () => {
  const date = new Date();
  date.setHours(0, 0, 0, 0);
  return date;
};

/** Invalidate date-dependent memoized views at midnight and after returning to the app. */
export function useLocalToday() {
  const [today, setToday] = useState(localToday);
  useEffect(() => {
    const refresh = () => {
      const next = localToday();
      setToday(previous => previous.getTime() === next.getTime() ? previous : next);
    };
    const timer = window.setInterval(refresh, 60_000);
    window.addEventListener('focus', refresh);
    document.addEventListener('visibilitychange', refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener('focus', refresh);
      document.removeEventListener('visibilitychange', refresh);
    };
  }, []);
  return today;
}

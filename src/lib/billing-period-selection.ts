import { useCallback, useEffect, useMemo, useState } from "react";
import { ipc } from "@/lib/ipc";
import type { BillingPeriod } from "@/lib/types";

const STORAGE_KEY = "selected-billing-period";
const EVENT_NAME = "billing-period-selection-changed";

type StoredSelection = {
  id: number | null;
  year: number | null;
  month: number | null;
};

function sortPeriods(periods: BillingPeriod[]) {
  return [...periods].sort((a, b) => {
    if (a.year !== b.year) return b.year - a.year;
    return b.month - a.month;
  });
}

function readStoredSelection(): StoredSelection | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as StoredSelection;
  } catch {
    return null;
  }
}

function findStoredPeriod(periods: BillingPeriod[], stored: StoredSelection | null) {
  if (!stored) return null;
  return (
    periods.find((period) => period.id === stored.id) ??
    periods.find(
      (period) => period.year === stored.year && period.month === stored.month,
    ) ??
    null
  );
}

export function setStoredBillingPeriod(period: BillingPeriod | null) {
  if (typeof window === "undefined") return;
  if (!period) {
    window.localStorage.removeItem(STORAGE_KEY);
  } else {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ id: period.id, year: period.year, month: period.month }),
    );
  }
  window.dispatchEvent(new Event(EVENT_NAME));
}

export function resolveStoredBillingPeriod(periods: BillingPeriod[]) {
  const ordered = sortPeriods(periods);
  const stored = findStoredPeriod(ordered, readStoredSelection());
  return stored ?? ordered[0] ?? null;
}

export function useBillingPeriodSelection() {
  const [allPeriods, setAllPeriods] = useState<BillingPeriod[]>([]);
  const [selectedYear, setSelectedYearState] = useState(new Date().getFullYear());
  const [selected, setSelectedState] = useState<BillingPeriod | null>(null);

  const applySelection = useCallback((period: BillingPeriod | null) => {
    if (!period) {
      setSelectedState(null);
      return;
    }
    setSelectedState(period);
    setSelectedYearState(period.year);
    setStoredBillingPeriod(period);
  }, []);

  const loadPeriods = useCallback(async () => {
    const periods = await ipc.getBillingPeriods();
    setAllPeriods(periods);
    const next = resolveStoredBillingPeriod(periods);
    if (next) {
      setSelectedState(next);
      setSelectedYearState(next.year);
      setStoredBillingPeriod(next);
    } else {
      setSelectedState(null);
    }
    return periods;
  }, []);

  const selectYear = useCallback(
    (year: number) => {
      setSelectedYearState(year);
      const periodsInYear = sortPeriods(
        allPeriods.filter((period) => period.year === year),
      );
      if (periodsInYear.length > 0) {
        applySelection(periodsInYear[0]);
      } else {
        setSelectedState(null);
      }
    },
    [allPeriods, applySelection],
  );

  useEffect(() => {
    void loadPeriods();
  }, [loadPeriods]);

  useEffect(() => {
    const handleSelectionChange = () => {
      const next = findStoredPeriod(allPeriods, readStoredSelection());
      if (next) {
        setSelectedState(next);
        setSelectedYearState(next.year);
      }
    };
    window.addEventListener(EVENT_NAME, handleSelectionChange);
    return () => window.removeEventListener(EVENT_NAME, handleSelectionChange);
  }, [allPeriods]);

  const years = useMemo(
    () => [...new Set(allPeriods.map((period) => period.year))].sort((a, b) => b - a),
    [allPeriods],
  );
  const yearPeriods = useMemo(
    () =>
      [...allPeriods]
        .filter((period) => period.year === selectedYear)
        .sort((a, b) => a.month - b.month),
    [allPeriods, selectedYear],
  );

  return {
    allPeriods,
    years,
    yearPeriods,
    selectedYear,
    selected,
    loadPeriods,
    setSelectedYear: selectYear,
    setSelected: applySelection,
  };
}

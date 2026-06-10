import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { ipc } from "@/lib/ipc";
import type { BillingPeriod } from "@/lib/types";

const STORAGE_KEY = "selected-billing-period";
const EVENT_NAME = "billing-period-selection-changed";

type StoredSelection = {
  id: number | null;
  year: number | null;
  month: number | null;
};

type BillingPeriodSelectionValue = {
  allPeriods: BillingPeriod[];
  years: number[];
  yearPeriods: BillingPeriod[];
  selectedYear: number;
  selected: BillingPeriod | null;
  loadPeriods: () => Promise<BillingPeriod[]>;
  setSelectedYear: (year: number) => void;
  setSelected: (period: BillingPeriod | null) => void;
};

const BillingPeriodSelectionContext =
  createContext<BillingPeriodSelectionValue | null>(null);

function sortPeriods(periods: BillingPeriod[]) {
  return [...periods].sort((a, b) => {
    if (a.year !== b.year) return b.year - a.year;
    return b.month - a.month;
  });
}

function findPreferredPeriodForYear(
  periods: BillingPeriod[],
  year: number,
  preferredMonth: number | null,
) {
  const periodsInYear = [...periods]
    .filter((period) => period.year === year)
    .sort((a, b) => a.month - b.month);

  if (periodsInYear.length === 0) return null;

  if (preferredMonth != null) {
    const sameMonth =
      periodsInYear.find((period) => period.month === preferredMonth) ?? null;
    if (sameMonth) return sameMonth;
  }

  return periodsInYear[0] ?? null;
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

async function periodHasWorkflowData(period: BillingPeriod) {
  if (period.id == null) return false;

  const [bills, splits] = await Promise.all([
    ipc.getBills(period.id),
    ipc.getSplits(period.id),
  ]);

  return bills.length > 0 || splits.length > 0;
}

async function resolveInitialBillingPeriod(periods: BillingPeriod[]) {
  const ordered = sortPeriods(periods);
  const stored = findStoredPeriod(ordered, readStoredSelection());

  if (stored && (await periodHasWorkflowData(stored))) {
    return stored;
  }

  const activity = await Promise.all(
    ordered.map(async (period) => ({
      period,
      hasData: await periodHasWorkflowData(period),
    })),
  );
  const latestWithData = activity.find(({ hasData }) => hasData)?.period ?? null;
  if (latestWithData) return latestWithData;

  const now = new Date();
  const currentPeriod =
    ordered.find(
      (period) =>
        period.year === now.getFullYear() && period.month === now.getMonth() + 1,
    ) ?? null;

  return currentPeriod ?? ordered[0] ?? null;
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

export function BillingPeriodSelectionProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [allPeriods, setAllPeriods] = useState<BillingPeriod[]>([]);
  const [selectedYear, setSelectedYearState] = useState(new Date().getFullYear());
  const [selected, setSelectedState] = useState<BillingPeriod | null>(null);
  const selectedRef = useRef<BillingPeriod | null>(null);

  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);

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

    const currentSelected = selectedRef.current;
    const next =
      (currentSelected
        ? periods.find((period) => period.id === currentSelected.id) ?? null
        : null) ?? (await resolveInitialBillingPeriod(periods));

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
      const next = findPreferredPeriodForYear(
        allPeriods,
        year,
        selected?.month ?? null,
      );
      if (next) {
        applySelection(next);
      } else {
        setSelectedState(null);
      }
    },
    [allPeriods, applySelection, selected],
  );

  useEffect(() => {
    void loadPeriods();
  }, [loadPeriods]);

  useEffect(() => {
    const handleSelectionChange = () => {
      const next =
        findStoredPeriod(allPeriods, readStoredSelection()) ??
        resolveStoredBillingPeriod(allPeriods);
      if (next) {
        setSelectedState(next);
        setSelectedYearState(next.year);
      }
    };
    window.addEventListener(EVENT_NAME, handleSelectionChange);
    return () => window.removeEventListener(EVENT_NAME, handleSelectionChange);
  }, [allPeriods]);

  const years = useMemo(
    () => [...new Set(allPeriods.map((period) => period.year))].sort((a, b) => a - b),
    [allPeriods],
  );
  const yearPeriods = useMemo(
    () =>
      [...allPeriods]
        .filter((period) => period.year === selectedYear)
        .sort((a, b) => a.month - b.month),
    [allPeriods, selectedYear],
  );

  const value = useMemo(
    () => ({
      allPeriods,
      years,
      yearPeriods,
      selectedYear,
      selected,
      loadPeriods,
      setSelectedYear: selectYear,
      setSelected: applySelection,
    }),
    [
      allPeriods,
      years,
      yearPeriods,
      selectedYear,
      selected,
      loadPeriods,
      selectYear,
      applySelection,
    ],
  );

  return (
    <BillingPeriodSelectionContext.Provider value={value}>
      {children}
    </BillingPeriodSelectionContext.Provider>
  );
}

export function useBillingPeriodSelection() {
  const context = useContext(BillingPeriodSelectionContext);
  if (!context) {
    throw new Error(
      "useBillingPeriodSelection must be used inside BillingPeriodSelectionProvider.",
    );
  }
  return context;
}

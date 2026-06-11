import {
  createContext,
  createElement,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { ipc } from "@/lib/ipc";
import type { Apartment, Bill, BillingPeriod, Building, Provider, SplitRow } from "@/lib/types";

const STORAGE_KEY = "selected-billing-period";
const EVENT_NAME = "billing-period-selection-changed";

type StoredSelection = {
  id: number | null;
  year: number | null;
  month: number | null;
};

export type PeriodStatus = {
  bills: boolean;
  splits: boolean;
  sent: boolean;
  billCount: number;
  splitCount: number;
  totalCents: number;
  needsReview: number;
};

export type WorkflowRefreshOptions = {
  periods?: boolean;
  core?: boolean;
  selected?: boolean;
  statuses?: boolean;
};

export type BillingPeriodSelectionValue = {
  allPeriods: BillingPeriod[];
  years: number[];
  yearPeriods: BillingPeriod[];
  selectedYear: number;
  selected: BillingPeriod | null;
  loadPeriods: () => Promise<BillingPeriod[]>;
  setSelectedYear: (year: number) => void;
  setSelected: (period: BillingPeriod | null) => void;
};

export type WorkflowSnapshot = {
  loading: boolean;
  isRefreshing: boolean;
  error: string | null;
  building: Building | null;
  buildingName: string;
  buildingCity: string;
  apartments: Apartment[];
  providers: Provider[];
  bills: Bill[];
  splits: SplitRow[];
  periodStatuses: Map<number, PeriodStatus>;
  selectedStatus: PeriodStatus;
  refresh: (options?: WorkflowRefreshOptions) => Promise<void>;
};

type PeriodRows = {
  bills: Bill[];
  splits: SplitRow[];
};

const BillingPeriodSelectionContext =
  createContext<BillingPeriodSelectionValue | null>(null);
const WorkflowSnapshotContext = createContext<WorkflowSnapshot | null>(null);

export const EMPTY_PERIOD_STATUS: PeriodStatus = {
  bills: false,
  splits: false,
  sent: false,
  billCount: 0,
  splitCount: 0,
  totalCents: 0,
  needsReview: 0,
};

function summarizePeriod(bills: Bill[], splits: SplitRow[]): PeriodStatus {
  return {
    bills: bills.length > 0,
    splits: splits.length > 0,
    sent: false,
    billCount: bills.length,
    splitCount: splits.length,
    totalCents: bills.reduce((sum, bill) => sum + bill.amount_cents, 0),
    needsReview: bills.filter((bill) => bill.parse_note?.trim()).length,
  };
}

function sortPeriods(periods: BillingPeriod[]) {
  return [...periods].sort((a, b) => {
    if (a.year !== b.year) return b.year - a.year;
    return b.month - a.month;
  });
}

function periodsWithIds(periods: BillingPeriod[]) {
  return periods.filter(
    (period): period is BillingPeriod & { id: number } => period.id != null,
  );
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

function periodHasWorkflowData(status: PeriodStatus | undefined) {
  return !!status && (status.bills || status.splits);
}

function resolveInitialBillingPeriod(
  periods: BillingPeriod[],
  statuses: Map<number, PeriodStatus>,
) {
  const ordered = sortPeriods(periods);
  const stored = findStoredPeriod(ordered, readStoredSelection());

  if (stored?.id != null && periodHasWorkflowData(statuses.get(stored.id))) {
    return stored;
  }

  const latestWithData =
    ordered.find(
      (period) => period.id != null && periodHasWorkflowData(statuses.get(period.id)),
    ) ?? null;
  if (latestWithData) return latestWithData;

  const now = new Date();
  const currentPeriod =
    ordered.find(
      (period) =>
        period.year === now.getFullYear() && period.month === now.getMonth() + 1,
    ) ?? null;

  return currentPeriod ?? ordered[0] ?? null;
}

async function fetchPeriodRows(periodId: number): Promise<PeriodRows> {
  const [bills, splits] = await Promise.all([
    ipc.getBills(periodId),
    ipc.getSplits(periodId),
  ]);
  return { bills, splits };
}

async function fetchPeriodStatuses(periods: BillingPeriod[]) {
  const rowsByPeriod = new Map<number, PeriodRows>();
  const entries = await Promise.all(
    periodsWithIds(periods).map(async (period) => {
      const rows = await fetchPeriodRows(period.id);
      rowsByPeriod.set(period.id, rows);
      return [period.id, summarizePeriod(rows.bills, rows.splits)] as const;
    }),
  );

  return {
    statuses: new Map(entries),
    rowsByPeriod,
  };
}

function writeStoredBillingPeriod(period: BillingPeriod | null, notify: boolean) {
  if (typeof window === "undefined") return;
  if (!period) {
    window.localStorage.removeItem(STORAGE_KEY);
  } else {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ id: period.id, year: period.year, month: period.month }),
    );
  }
  if (notify) window.dispatchEvent(new Event(EVENT_NAME));
}

export function setStoredBillingPeriod(period: BillingPeriod | null) {
  writeStoredBillingPeriod(period, true);
}

export function resolveStoredBillingPeriod(periods: BillingPeriod[]) {
  const ordered = sortPeriods(periods);
  const stored = findStoredPeriod(ordered, readStoredSelection());
  return stored ?? ordered[0] ?? null;
}

export function WorkflowSnapshotProvider({ children }: { children: ReactNode }) {
  const [initialLoading, setInitialLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [selectedLoading, setSelectedLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [allPeriods, setAllPeriods] = useState<BillingPeriod[]>([]);
  const [selectedYear, setSelectedYearState] = useState(new Date().getFullYear());
  const [selected, setSelectedState] = useState<BillingPeriod | null>(null);
  const [building, setBuilding] = useState<Building | null>(null);
  const [buildingName, setBuildingName] = useState("Kamniska ulica 36");
  const [buildingCity, setBuildingCity] = useState("Ljubljana");
  const [apartments, setApartments] = useState<Apartment[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [bills, setBills] = useState<Bill[]>([]);
  const [splits, setSplits] = useState<SplitRow[]>([]);
  const [selectedDataPeriodId, setSelectedDataPeriodId] = useState<number | null>(null);
  const [periodStatuses, setPeriodStatuses] = useState<Map<number, PeriodStatus>>(
    () => new Map(),
  );
  const requestRef = useRef(0);
  const selectedRequestRef = useRef(0);
  const hasLoadedRef = useRef(false);
  const periodsRef = useRef<BillingPeriod[]>([]);
  const selectedRef = useRef<BillingPeriod | null>(null);
  const selectedDataPeriodIdRef = useRef<number | null>(null);
  const periodStatusesRef = useRef<Map<number, PeriodStatus>>(new Map());

  useEffect(() => {
    periodsRef.current = allPeriods;
  }, [allPeriods]);

  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);

  useEffect(() => {
    periodStatusesRef.current = periodStatuses;
  }, [periodStatuses]);

  useEffect(() => {
    selectedDataPeriodIdRef.current = selectedDataPeriodId;
  }, [selectedDataPeriodId]);

  const commitBuilding = useCallback((nextBuilding: Building) => {
    setBuilding(nextBuilding);
    setBuildingName(nextBuilding.name || "Kamniska ulica 36");
    setBuildingCity(nextBuilding.city || "Ljubljana");
  }, []);

  const refreshSelectedPeriod = useCallback(
    async (period: BillingPeriod | null, rowsFromStatus?: PeriodRows) => {
      const requestId = ++selectedRequestRef.current;

      if (period?.id == null) {
        setSelectedLoading(false);
        setBills([]);
        setSplits([]);
        setSelectedDataPeriodId(null);
        return;
      }

      setSelectedLoading(true);
      try {
        const rows = rowsFromStatus ?? (await fetchPeriodRows(period.id));
        if (selectedRequestRef.current !== requestId) return;
        setError(null);
        setBills(rows.bills);
        setSplits(rows.splits);
        setSelectedDataPeriodId(period.id);
        setPeriodStatuses((current) => {
          const next = new Map(current);
          next.set(period.id!, summarizePeriod(rows.bills, rows.splits));
          return next;
        });
      } catch (error) {
        if (selectedRequestRef.current !== requestId) return;
        setError(String(error));
        if (selectedDataPeriodIdRef.current !== period.id) {
          setBills([]);
          setSplits([]);
          setSelectedDataPeriodId(period.id);
        }
      } finally {
        if (selectedRequestRef.current === requestId) setSelectedLoading(false);
      }
    },
    [],
  );

  const applySelection = useCallback(
    (period: BillingPeriod | null, rowsFromStatus?: PeriodRows) => {
      if (!period) {
        setSelectedState(null);
        writeStoredBillingPeriod(null, false);
        void refreshSelectedPeriod(null);
        return;
      }

      setSelectedState(period);
      setSelectedYearState(period.year);
      writeStoredBillingPeriod(period, false);
      void refreshSelectedPeriod(period, rowsFromStatus);
    },
    [refreshSelectedPeriod],
  );

  const refresh = useCallback(
    async (options: WorkflowRefreshOptions = {}) => {
      const requestId = ++requestRef.current;
      const scopes = {
        periods: options.periods ?? true,
        core: options.core ?? true,
        selected: options.selected ?? true,
        statuses: options.statuses ?? true,
      };
      const initial = !hasLoadedRef.current;

      if (initial) {
        setInitialLoading(true);
      } else {
        setRefreshing(true);
      }

      try {
        const [periodsResult, coreResult] = await Promise.all([
          scopes.periods ? ipc.getBillingPeriods() : Promise.resolve(periodsRef.current),
          scopes.core
            ? Promise.all([ipc.getBuilding(), ipc.getApartments(), ipc.getProviders()])
            : Promise.resolve(null),
        ]);
        if (requestRef.current !== requestId) return;

        const nextPeriods = periodsResult;
        let rowsByPeriod = new Map<number, PeriodRows>();
        let nextStatuses = periodStatusesRef.current;

        if (scopes.statuses) {
          const statusResult = await fetchPeriodStatuses(nextPeriods);
          if (requestRef.current !== requestId) return;
          rowsByPeriod = statusResult.rowsByPeriod;
          nextStatuses = statusResult.statuses;
          setPeriodStatuses(nextStatuses);
        }

        if (scopes.periods) {
          setAllPeriods(nextPeriods);
        }

        if (coreResult) {
          const [nextBuilding, nextApartments, nextProviders] = coreResult;
          commitBuilding(nextBuilding);
          setApartments(nextApartments);
          setProviders(nextProviders);
        }

        let nextSelected = selectedRef.current;
        if (scopes.periods) {
          const selectedInNextPeriods =
            nextSelected?.id != null
              ? nextPeriods.find((period) => period.id === nextSelected?.id) ?? null
              : null;
          nextSelected = selectedInNextPeriods ?? resolveInitialBillingPeriod(nextPeriods, nextStatuses);

          if (nextSelected) {
            setSelectedState(nextSelected);
            setSelectedYearState(nextSelected.year);
            writeStoredBillingPeriod(nextSelected, false);
          } else {
            setSelectedState(null);
            writeStoredBillingPeriod(null, false);
          }
        }

        if (scopes.selected) {
          await refreshSelectedPeriod(
            nextSelected,
            nextSelected?.id != null ? rowsByPeriod.get(nextSelected.id) : undefined,
          );
        }

        hasLoadedRef.current = true;
      } catch (error) {
        if (requestRef.current === requestId) setError(String(error));
      } finally {
        if (requestRef.current === requestId) {
          setInitialLoading(false);
          setRefreshing(false);
        }
      }
    },
    [commitBuilding, refreshSelectedPeriod],
  );

  const loadPeriods = useCallback(async () => {
    const periods = await ipc.getBillingPeriods();
    const { statuses, rowsByPeriod } = await fetchPeriodStatuses(periods);
    setAllPeriods(periods);
    setPeriodStatuses(statuses);

    const currentSelected = selectedRef.current;
    const next =
      (currentSelected?.id != null
        ? periods.find((period) => period.id === currentSelected.id) ?? null
        : null) ?? resolveInitialBillingPeriod(periods, statuses);

    if (next) {
      setSelectedState(next);
      setSelectedYearState(next.year);
      writeStoredBillingPeriod(next, false);
      await refreshSelectedPeriod(
        next,
        next.id != null ? rowsByPeriod.get(next.id) : undefined,
      );
    } else {
      setSelectedState(null);
      writeStoredBillingPeriod(null, false);
      await refreshSelectedPeriod(null);
    }

    return periods;
  }, [refreshSelectedPeriod]);

  const selectYear = useCallback(
    (year: number) => {
      setSelectedYearState(year);
      const next = findPreferredPeriodForYear(
        periodsRef.current,
        year,
        selectedRef.current?.month ?? null,
      );
      if (next) {
        applySelection(next);
      } else {
        setSelectedState(null);
        void refreshSelectedPeriod(null);
      }
    },
    [applySelection, refreshSelectedPeriod],
  );

  useEffect(() => {
    void refresh();
    return () => {
      requestRef.current += 1;
      selectedRequestRef.current += 1;
    };
  }, [refresh]);

  useEffect(() => {
    const handleSelectionChange = () => {
      const next =
        findStoredPeriod(periodsRef.current, readStoredSelection()) ??
        resolveStoredBillingPeriod(periodsRef.current);
      if (next) {
        setSelectedState(next);
        setSelectedYearState(next.year);
        void refreshSelectedPeriod(next);
      }
    };
    window.addEventListener(EVENT_NAME, handleSelectionChange);
    return () => window.removeEventListener(EVENT_NAME, handleSelectionChange);
  }, [refreshSelectedPeriod]);

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
  const selectedStatus = useMemo(
    () =>
      selected?.id == null
        ? EMPTY_PERIOD_STATUS
        : periodStatuses.get(selected.id) ?? EMPTY_PERIOD_STATUS,
    [periodStatuses, selected],
  );
  const selectedDataFresh = selected?.id == null || selectedDataPeriodId === selected.id;
  const exposedBills = selectedDataFresh ? bills : [];
  const exposedSplits = selectedDataFresh ? splits : [];
  const loading = initialLoading || !selectedDataFresh;

  const billingPeriodSelection = useMemo<BillingPeriodSelectionValue>(
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

  const snapshot = useMemo<WorkflowSnapshot>(
    () => ({
      loading,
      isRefreshing: refreshing || selectedLoading,
      error,
      building,
      buildingName,
      buildingCity,
      apartments,
      providers,
      bills: exposedBills,
      splits: exposedSplits,
      periodStatuses,
      selectedStatus,
      refresh,
    }),
    [
      loading,
      refreshing,
      selectedLoading,
      error,
      building,
      buildingName,
      buildingCity,
      apartments,
      providers,
      exposedBills,
      exposedSplits,
      periodStatuses,
      selectedStatus,
      refresh,
    ],
  );

  return createElement(
    BillingPeriodSelectionContext.Provider,
    { value: billingPeriodSelection },
    createElement(
      WorkflowSnapshotContext.Provider,
      { value: snapshot },
      children,
    ),
  );
}

export const BillingPeriodSelectionProvider = WorkflowSnapshotProvider;

export function useWorkflowSnapshotContext() {
  const snapshot = useContext(WorkflowSnapshotContext);
  if (!snapshot) {
    throw new Error(
      "useWorkflowSnapshotContext must be used inside WorkflowSnapshotProvider.",
    );
  }
  return snapshot;
}

export function useBillingPeriodSelection() {
  const context = useContext(BillingPeriodSelectionContext);
  if (!context) {
    throw new Error(
      "useBillingPeriodSelection must be used inside WorkflowSnapshotProvider.",
    );
  }
  return context;
}

export function useWorkflowSnapshot() {
  return useWorkflowSnapshotContext();
}

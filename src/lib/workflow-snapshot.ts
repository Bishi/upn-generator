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
import { subscribeWorkflowStatusChanged } from "@/lib/workflow-status";
import type { Apartment, Bill, BillingPeriod, Building, Provider, SplitRow } from "@/lib/types";

export type PeriodStatus = {
  bills: boolean;
  splits: boolean;
  sent: boolean;
  billCount: number;
  splitCount: number;
  totalCents: number;
  needsReview: number;
};

export type WorkflowSnapshot = {
  loading: boolean;
  building: Building | null;
  buildingName: string;
  buildingCity: string;
  apartments: Apartment[];
  providers: Provider[];
  bills: Bill[];
  splits: SplitRow[];
  periodStatuses: Map<number, PeriodStatus>;
  selectedStatus: PeriodStatus;
  refresh: () => Promise<void>;
};

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

export function useWorkflowSnapshot(
  selectedPeriodId: number | null | undefined,
  periods: BillingPeriod[] = [],
): WorkflowSnapshot {
  const [loading, setLoading] = useState(true);
  const [building, setBuilding] = useState<Building | null>(null);
  const [buildingName, setBuildingName] = useState("Kamniska ulica 36");
  const [buildingCity, setBuildingCity] = useState("Ljubljana");
  const [apartments, setApartments] = useState<Apartment[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [bills, setBills] = useState<Bill[]>([]);
  const [splits, setSplits] = useState<SplitRow[]>([]);
  const [periodStatuses, setPeriodStatuses] = useState<Map<number, PeriodStatus>>(
    () => new Map(),
  );
  const requestRef = useRef(0);

  const refresh = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);

    try {
      const [building, nextApartments, nextProviders] = await Promise.all([
        ipc.getBuilding(),
        ipc.getApartments(),
        ipc.getProviders(),
      ]);
      if (requestRef.current !== requestId) return;

      setBuilding(building);
      setBuildingName(building.name || "Kamniska ulica 36");
      setBuildingCity(building.city || "Ljubljana");
      setApartments(nextApartments);
      setProviders(nextProviders);

      const periodsWithIds = periods.filter(
        (period): period is BillingPeriod & { id: number } => period.id != null,
      );
      const selectedKnown =
        selectedPeriodId != null &&
        periodsWithIds.some((period) => period.id === selectedPeriodId);
      const statusPeriods = selectedKnown
        ? periodsWithIds
        : selectedPeriodId != null
          ? [{ id: selectedPeriodId } as BillingPeriod & { id: number }, ...periodsWithIds]
          : periodsWithIds;

      const entries = await Promise.all(
        statusPeriods.map(async (period) => {
          const [periodBills, periodSplits] = await Promise.all([
            ipc.getBills(period.id),
            ipc.getSplits(period.id),
          ]);
          return [period.id, summarizePeriod(periodBills, periodSplits)] as const;
        }),
      );
      if (requestRef.current !== requestId) return;

      const nextStatuses = new Map(entries);
      setPeriodStatuses(nextStatuses);

      if (selectedPeriodId != null) {
        const [selectedBills, selectedSplits] = await Promise.all([
          ipc.getBills(selectedPeriodId),
          ipc.getSplits(selectedPeriodId),
        ]);
        if (requestRef.current !== requestId) return;
        setBills(selectedBills);
        setSplits(selectedSplits);
        nextStatuses.set(selectedPeriodId, summarizePeriod(selectedBills, selectedSplits));
        setPeriodStatuses(new Map(nextStatuses));
      } else {
        setBills([]);
        setSplits([]);
      }
    } finally {
      if (requestRef.current === requestId) setLoading(false);
    }
  }, [periods, selectedPeriodId]);

  useEffect(() => {
    void refresh();
    return () => {
      requestRef.current += 1;
    };
  }, [refresh]);

  useEffect(() => subscribeWorkflowStatusChanged(() => void refresh()), [refresh]);

  const selectedStatus = useMemo(
    () =>
      selectedPeriodId == null
        ? EMPTY_PERIOD_STATUS
        : periodStatuses.get(selectedPeriodId) ?? EMPTY_PERIOD_STATUS,
    [periodStatuses, selectedPeriodId],
  );

  return {
    loading,
    building,
    buildingName,
    buildingCity,
    apartments,
    providers,
    bills,
    splits,
    periodStatuses,
    selectedStatus,
    refresh,
  };
}

export function WorkflowSnapshotProvider({
  selectedPeriodId,
  periods,
  children,
}: {
  selectedPeriodId: number | null | undefined;
  periods: BillingPeriod[];
  children: ReactNode;
}) {
  const snapshot = useWorkflowSnapshot(selectedPeriodId, periods);

  return createElement(
    WorkflowSnapshotContext.Provider,
    { value: snapshot },
    children,
  );
}

export function useWorkflowSnapshotContext() {
  const snapshot = useContext(WorkflowSnapshotContext);
  if (!snapshot) {
    throw new Error(
      "useWorkflowSnapshotContext must be used inside WorkflowSnapshotProvider.",
    );
  }
  return snapshot;
}

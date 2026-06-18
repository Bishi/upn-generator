export type SettingsTabId = "building" | "apartments" | "providers" | "delivery" | "app";

export interface SettingsDirtyEntry {
  tab: SettingsTabId;
  label: string;
  isDirty: boolean;
  isBusy?: boolean;
  discard: () => void;
}

export type SettingsDirtyRegistrar = (
  id: string,
  entry: SettingsDirtyEntry,
) => () => void;

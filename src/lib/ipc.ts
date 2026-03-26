import { invoke } from "@tauri-apps/api/core";
import type { Apartment, Building, Provider, SmtpConfig } from "./types";

export const ipc = {
  // Building
  getBuilding: () => invoke<Building>("get_building"),
  saveBuilding: (building: Building) => invoke<Building>("save_building", { building }),

  // Apartments
  getApartments: () => invoke<Apartment[]>("get_apartments"),
  saveApartment: (apartment: Apartment) => invoke<Apartment>("save_apartment", { apartment }),
  deleteApartment: (id: number) => invoke<void>("delete_apartment", { id }),

  // Providers
  getProviders: () => invoke<Provider[]>("get_providers"),
  saveProvider: (provider: Provider) => invoke<Provider>("save_provider", { provider }),
  deleteProvider: (id: number) => invoke<void>("delete_provider", { id }),

  // SMTP
  getSmtpConfig: () => invoke<SmtpConfig>("get_smtp_config"),
  saveSmtpConfig: (config: SmtpConfig) => invoke<void>("save_smtp_config", { config }),
};

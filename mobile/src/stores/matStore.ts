import { create } from 'zustand';
import type { Alert, DisasterEvent, ScanZone, Survivor } from '@/types/mat';

export interface MatState {
  events: DisasterEvent[];
  zones: ScanZone[];
  survivors: Survivor[];
  alerts: Alert[];
  selectedEventId: string | null;
  upsertEvent: (event: DisasterEvent) => void;
  addZone: (zone: ScanZone) => void;
  upsertSurvivor: (survivor: Survivor) => void;
  addAlert: (alert: Alert) => void;
  setSelectedEvent: (id: string | null) => void;
}

export const useMatStore = create<MatState>((set) => ({
  events: [],
  zones: [],
  survivors: [],
  alerts: [],
  selectedEventId: null,

  upsertEvent: (event) => {
    set((state) => {
      const index = state.events.findIndex((item) => item.event_id === event.event_id);
      if (index === -1) {
        return { events: [...state.events, event] };
      }
      const events = [...state.events];
      events[index] = event;
      return { events };
    });
  },

  addZone: (zone) => {
    set((state) => {
      const index = state.zones.findIndex((item) => item.id === zone.id);
      if (index === -1) {
        return { zones: [...state.zones, zone] };
      }
      const zones = [...state.zones];
      zones[index] = zone;
      return { zones };
    });
  },

  upsertSurvivor: (survivor) => {
    set((state) => {
      const index = state.survivors.findIndex((item) => item.id === survivor.id);
      if (index === -1) {
        return { survivors: [...state.survivors, survivor] };
      }
      const survivors = [...state.survivors];
      survivors[index] = survivor;
      return { survivors };
    });
  },

  addAlert: (alert) => {
    set((state) => {
      if (state.alerts.some((item) => item.id === alert.id)) {
        return {
          alerts: state.alerts.map((item) => (item.id === alert.id ? alert : item)),
        };
      }
      return { alerts: [...state.alerts, alert] };
    });
  },

  setSelectedEvent: (id) => {
    set({ selectedEventId: id });
  },
}));

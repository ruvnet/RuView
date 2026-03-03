import { useMatStore } from '@/stores/matStore';
import { AlertPriority, TriageStatus, ZoneStatus } from '@/types/mat';
import type { Alert, DisasterEvent, ScanZone, Survivor } from '@/types/mat';

const makeEvent = (overrides: Partial<DisasterEvent> = {}): DisasterEvent => ({
  event_id: 'evt-1',
  disaster_type: 1,
  latitude: 37.77,
  longitude: -122.41,
  description: 'Earthquake in SF',
  ...overrides,
});

const makeZone = (overrides: Partial<ScanZone> = {}): ScanZone => ({
  id: 'zone-1',
  name: 'Zone A',
  zone_type: 'rectangle',
  status: ZoneStatus.Active,
  scan_count: 0,
  detection_count: 0,
  bounds_json: '{}',
  ...overrides,
} as ScanZone);

const makeSurvivor = (overrides: Partial<Survivor> = {}): Survivor => ({
  id: 'surv-1',
  zone_id: 'zone-1',
  x: 100,
  y: 150,
  depth: 2.5,
  triage_status: TriageStatus.Immediate,
  triage_color: '#FF0000',
  confidence: 0.9,
  breathing_rate: 16,
  heart_rate: 80,
  first_detected: '2024-01-01T00:00:00Z',
  last_updated: '2024-01-01T00:01:00Z',
  is_deteriorating: false,
  ...overrides,
});

const makeAlert = (overrides: Partial<Alert> = {}): Alert => ({
  id: 'alert-1',
  survivor_id: 'surv-1',
  priority: AlertPriority.Critical,
  title: 'Critical survivor',
  message: 'Breathing rate dropping',
  recommended_action: 'Immediate extraction',
  triage_status: TriageStatus.Immediate,
  location_x: 100,
  location_y: 150,
  created_at: '2024-01-01T00:01:00Z',
  priority_color: '#FF0000',
  ...overrides,
});

describe('useMatStore', () => {
  beforeEach(() => {
    useMatStore.setState({
      events: [],
      zones: [],
      survivors: [],
      alerts: [],
      selectedEventId: null,
    });
  });

  describe('initial state', () => {
    it('has empty events array', () => {
      expect(useMatStore.getState().events).toEqual([]);
    });

    it('has empty zones array', () => {
      expect(useMatStore.getState().zones).toEqual([]);
    });

    it('has empty survivors array', () => {
      expect(useMatStore.getState().survivors).toEqual([]);
    });

    it('has empty alerts array', () => {
      expect(useMatStore.getState().alerts).toEqual([]);
    });

    it('has null selectedEventId', () => {
      expect(useMatStore.getState().selectedEventId).toBeNull();
    });
  });

  describe('upsertEvent', () => {
    it('adds a new event', () => {
      const event = makeEvent();
      useMatStore.getState().upsertEvent(event);
      expect(useMatStore.getState().events).toEqual([event]);
    });

    it('updates an existing event by event_id', () => {
      const event = makeEvent();
      useMatStore.getState().upsertEvent(event);

      const updated = makeEvent({ description: 'Updated description' });
      useMatStore.getState().upsertEvent(updated);

      const events = useMatStore.getState().events;
      expect(events).toHaveLength(1);
      expect(events[0].description).toBe('Updated description');
    });

    it('adds a second event with different event_id', () => {
      useMatStore.getState().upsertEvent(makeEvent({ event_id: 'evt-1' }));
      useMatStore.getState().upsertEvent(makeEvent({ event_id: 'evt-2' }));
      expect(useMatStore.getState().events).toHaveLength(2);
    });
  });

  describe('addZone', () => {
    it('adds a new zone', () => {
      const zone = makeZone();
      useMatStore.getState().addZone(zone);
      expect(useMatStore.getState().zones).toEqual([zone]);
    });

    it('updates an existing zone by id', () => {
      const zone = makeZone();
      useMatStore.getState().addZone(zone);

      const updated = makeZone({ name: 'Zone A Updated', scan_count: 5 });
      useMatStore.getState().addZone(updated);

      const zones = useMatStore.getState().zones;
      expect(zones).toHaveLength(1);
      expect(zones[0].name).toBe('Zone A Updated');
      expect(zones[0].scan_count).toBe(5);
    });

    it('adds multiple distinct zones', () => {
      useMatStore.getState().addZone(makeZone({ id: 'zone-1' }));
      useMatStore.getState().addZone(makeZone({ id: 'zone-2' }));
      expect(useMatStore.getState().zones).toHaveLength(2);
    });
  });

  describe('upsertSurvivor', () => {
    it('adds a new survivor', () => {
      const survivor = makeSurvivor();
      useMatStore.getState().upsertSurvivor(survivor);
      expect(useMatStore.getState().survivors).toEqual([survivor]);
    });

    it('updates an existing survivor by id', () => {
      useMatStore.getState().upsertSurvivor(makeSurvivor());
      const updated = makeSurvivor({ confidence: 0.95, is_deteriorating: true });
      useMatStore.getState().upsertSurvivor(updated);

      const survivors = useMatStore.getState().survivors;
      expect(survivors).toHaveLength(1);
      expect(survivors[0].confidence).toBe(0.95);
      expect(survivors[0].is_deteriorating).toBe(true);
    });
  });

  describe('addAlert', () => {
    it('adds a new alert', () => {
      const alert = makeAlert();
      useMatStore.getState().addAlert(alert);
      expect(useMatStore.getState().alerts).toEqual([alert]);
    });

    it('updates an existing alert by id', () => {
      useMatStore.getState().addAlert(makeAlert());
      const updated = makeAlert({ message: 'Updated message' });
      useMatStore.getState().addAlert(updated);

      const alerts = useMatStore.getState().alerts;
      expect(alerts).toHaveLength(1);
      expect(alerts[0].message).toBe('Updated message');
    });

    it('adds multiple distinct alerts', () => {
      useMatStore.getState().addAlert(makeAlert({ id: 'alert-1' }));
      useMatStore.getState().addAlert(makeAlert({ id: 'alert-2' }));
      expect(useMatStore.getState().alerts).toHaveLength(2);
    });
  });

  describe('setSelectedEvent', () => {
    it('sets the selected event id', () => {
      useMatStore.getState().setSelectedEvent('evt-1');
      expect(useMatStore.getState().selectedEventId).toBe('evt-1');
    });

    it('clears the selection with null', () => {
      useMatStore.getState().setSelectedEvent('evt-1');
      useMatStore.getState().setSelectedEvent(null);
      expect(useMatStore.getState().selectedEventId).toBeNull();
    });
  });
});

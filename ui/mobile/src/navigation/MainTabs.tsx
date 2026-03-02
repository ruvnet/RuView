import React, { Suspense } from 'react';
import { ActivityIndicator } from 'react-native';
import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { Ionicons } from '@expo/vector-icons';
import { ThemedText } from '../components/ThemedText';
import { ThemedView } from '../components/ThemedView';
import { colors } from '../theme/colors';
import { useMatStore } from '../stores/matStore';
import { MainTabsParamList } from './types';

const createPlaceholder = (label: string) => {
  const Placeholder = () => (
    <ThemedView style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
      <ThemedText preset="bodyLg">{label} screen not implemented yet</ThemedText>
      <ThemedText preset="bodySm" color="textSecondary">
        Placeholder shell
      </ThemedText>
    </ThemedView>
  );
  const LazyPlaceholder = React.lazy(async () => ({ default: Placeholder }));

  const Wrapped = () => (
    <Suspense
      fallback={
        <ThemedView style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
          <ActivityIndicator color={colors.accent} />
          <ThemedText preset="bodySm" color="textSecondary" style={{ marginTop: 8 }}>
            Loading {label}
          </ThemedText>
        </ThemedView>
      }
    >
      <LazyPlaceholder />
    </Suspense>
  );

  return Wrapped;
};

const wrapLazy = (
  loader: () => Promise<{ default: React.ComponentType }>,
  label: string,
) => {
  const fallback = createPlaceholder(label);
  return React.lazy(async () => {
    try {
      const module = await loader();
      if (module?.default) {
        return module;
      }
    } catch {
      // keep fallback for shell-only screens
    }
    return { default: fallback } as { default: React.ComponentType };
  });
};

const LiveScreen = wrapLazy(() => import('../screens/LiveScreen'), 'Live');
const VitalsScreen = wrapLazy(() => import('../screens/VitalsScreen'), 'Vitals');
const ZonesScreen = wrapLazy(() => import('../screens/ZonesScreen'), 'Zones');
const MATScreen = wrapLazy(() => import('../screens/MATScreen'), 'MAT');
const SettingsScreen = wrapLazy(() => import('../screens/SettingsScreen'), 'Settings');

const toIconName = (routeName: keyof MainTabsParamList) => {
  switch (routeName) {
    case 'Live':
      return 'wifi';
    case 'Vitals':
      return 'heart';
    case 'Zones':
      return 'grid';
    case 'MAT':
      return 'shield-checkmark';
    case 'Settings':
      return 'settings';
    default:
      return 'ellipse';
  }
};

const screens: ReadonlyArray<{ name: keyof MainTabsParamList; component: React.ComponentType }> = [
  { name: 'Live', component: LiveScreen },
  { name: 'Vitals', component: VitalsScreen },
  { name: 'Zones', component: ZonesScreen },
  { name: 'MAT', component: MATScreen },
  { name: 'Settings', component: SettingsScreen },
];

const Tab = createBottomTabNavigator<MainTabsParamList>();

const Suspended = ({ component: Component }: { component: React.ComponentType }) => (
  <Suspense fallback={<ActivityIndicator color={colors.accent} />}>
    <Component />
  </Suspense>
);

export const MainTabs = () => {
  const matAlertCount = useMatStore((state) => state.alerts.length);

  return (
    <Tab.Navigator
      screenOptions={({ route }) => ({
        headerShown: false,
        tabBarActiveTintColor: colors.accent,
        tabBarInactiveTintColor: colors.textSecondary,
        tabBarStyle: {
          backgroundColor: '#0D1117',
          borderTopColor: colors.border,
          borderTopWidth: 1,
        },
        tabBarIcon: ({ color, size }) => <Ionicons name={toIconName(route.name)} size={size} color={color} />,
        tabBarLabelStyle: {
          fontFamily: 'Courier New',
          textTransform: 'uppercase',
          fontSize: 10,
        },
        tabBarLabel: ({ children, color }) => <ThemedText style={{ color }}>{children}</ThemedText>,
      })}
    >
      {screens.map(({ name, component }) => (
        <Tab.Screen
          key={name}
          name={name}
          options={{
            tabBarBadge: name === 'MAT' ? (matAlertCount > 0 ? matAlertCount : undefined) : undefined,
          }}
          component={() => <Suspended component={component} />}
        />
      ))}
    </Tab.Navigator>
  );
};

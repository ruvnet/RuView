import { View } from 'react-native';
import { ThemedText } from '@/components/ThemedText';
import { colors } from '@/theme/colors';
import { spacing } from '@/theme/spacing';
import { AlertPriority, type Alert } from '@/types/mat';

type SeverityLevel = 'URGENT' | 'HIGH' | 'NORMAL';

type AlertCardProps = {
  alert: Alert;
};

type SeverityMeta = {
  label: SeverityLevel;
  icon: string;
  color: string;
};

const resolveSeverity = (alert: Alert): SeverityMeta => {
  if (alert.priority === AlertPriority.Critical) {
    return {
      label: 'URGENT',
      icon: '‼',
      color: colors.danger,
    };
  }

  if (alert.priority === AlertPriority.High) {
    return {
      label: 'HIGH',
      icon: '⚠',
      color: colors.warn,
    };
  }

  return {
    label: 'NORMAL',
    icon: '•',
    color: colors.accent,
  };
};

const formatTime = (value?: string): string => {
  if (!value) {
    return 'Unknown';
  }

  try {
    return new Date(value).toLocaleTimeString();
  } catch {
    return 'Unknown';
  }
};

export const AlertCard = ({ alert }: AlertCardProps) => {
  const severity = resolveSeverity(alert);

  return (
    <View
      style={{
        backgroundColor: '#111827',
        borderWidth: 1,
        borderColor: `${severity.color}55`,
        padding: spacing.md,
        borderRadius: 10,
        marginBottom: spacing.sm,
      }}
    >
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <ThemedText preset="labelMd" style={{ color: severity.color }}>
          {severity.icon} {severity.label}
        </ThemedText>
        <View style={{ flex: 1 }}>
          <ThemedText preset="bodySm" style={{ color: colors.textSecondary }}>
            {formatTime(alert.created_at)}
          </ThemedText>
        </View>
      </View>
      <ThemedText preset="bodyMd" style={{ color: colors.textPrimary, marginTop: 6 }}>
        {alert.message}
      </ThemedText>
    </View>
  );
};

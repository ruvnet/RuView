import { FlatList, View } from 'react-native';
import { ThemedText } from '@/components/ThemedText';
import { colors } from '@/theme/colors';
import { spacing } from '@/theme/spacing';
import type { Alert } from '@/types/mat';
import { AlertCard } from './AlertCard';

type AlertListProps = {
  alerts: Alert[];
};

export const AlertList = ({ alerts }: AlertListProps) => {
  if (alerts.length === 0) {
    return (
      <View
        style={{
          alignItems: 'center',
          justifyContent: 'center',
          padding: spacing.md,
          borderWidth: 1,
          borderColor: colors.border,
          borderRadius: 12,
          backgroundColor: '#111827',
        }}
      >
        <ThemedText preset="bodyMd">No alerts — system nominal</ThemedText>
      </View>
    );
  }

  return (
    <FlatList
      data={alerts}
      keyExtractor={(item) => item.id}
      renderItem={({ item }) => <AlertCard alert={item} />}
      contentContainerStyle={{ paddingBottom: spacing.md }}
      showsVerticalScrollIndicator={false}
      removeClippedSubviews={false}
    />
  );
};

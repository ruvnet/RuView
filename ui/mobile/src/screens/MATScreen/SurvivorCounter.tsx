import { View } from 'react-native';
import { ThemedText } from '@/components/ThemedText';
import { colors } from '@/theme/colors';
import { spacing } from '@/theme/spacing';
import { TriageStatus, type Survivor } from '@/types/mat';

type SurvivorCounterProps = {
  survivors: Survivor[];
};

type Breakdown = {
  immediate: number;
  delayed: number;
  minor: number;
  deceased: number;
  unknown: number;
};

const getBreakdown = (survivors: Survivor[]): Breakdown => {
  const output = {
    immediate: 0,
    delayed: 0,
    minor: 0,
    deceased: 0,
    unknown: 0,
  };

  survivors.forEach((survivor) => {
    if (survivor.triage_status === TriageStatus.Immediate) {
      output.immediate += 1;
      return;
    }
    if (survivor.triage_status === TriageStatus.Delayed) {
      output.delayed += 1;
      return;
    }
    if (survivor.triage_status === TriageStatus.Minor) {
      output.minor += 1;
      return;
    }
    if (survivor.triage_status === TriageStatus.Deceased) {
      output.deceased += 1;
      return;
    }

    output.unknown += 1;
  });

  return output;
};

const BreakoutChip = ({ label, value, color }: { label: string; value: number; color: string }) => (
  <View
    style={{
      backgroundColor: '#0D1117',
      borderRadius: 999,
      borderWidth: 1,
      borderColor: `${color}55`,
      paddingHorizontal: spacing.sm,
      paddingVertical: 4,
      marginRight: spacing.sm,
      marginTop: spacing.sm,
    }}
  >
    <ThemedText preset="bodySm" style={{ color }}>
      {label}: {value}
    </ThemedText>
  </View>
);

export const SurvivorCounter = ({ survivors }: SurvivorCounterProps) => {
  const total = survivors.length;
  const breakdown = getBreakdown(survivors);

  return (
    <View style={{ paddingBottom: spacing.md }}>
      <ThemedText preset="displayLg" style={{ color: colors.textPrimary }}>
        {total} SURVIVORS DETECTED
      </ThemedText>
      <View style={{ flexDirection: 'row', flexWrap: 'wrap', marginTop: spacing.sm }}>
        <BreakoutChip label="Immediate" value={breakdown.immediate} color={colors.danger} />
        <BreakoutChip label="Delayed" value={breakdown.delayed} color={colors.warn} />
        <BreakoutChip label="Minimal" value={breakdown.minor} color={colors.success} />
        <BreakoutChip label="Expectant" value={breakdown.deceased} color={colors.textSecondary} />
        <BreakoutChip label="Unknown" value={breakdown.unknown} color="#a0aec0" />
      </View>
    </View>
  );
};

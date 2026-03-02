import { useState } from 'react';
import { Pressable, TextInput, View } from 'react-native';
import { validateServerUrl } from '@/utils/urlValidator';
import { apiService } from '@/services/api.service';
import { ThemedText } from '@/components/ThemedText';
import { colors } from '@/theme/colors';
import { spacing } from '@/theme/spacing';

type ServerUrlInputProps = {
  value: string;
  onChange: (value: string) => void;
  onSave: () => void;
};

export const ServerUrlInput = ({ value, onChange, onSave }: ServerUrlInputProps) => {
  const [testResult, setTestResult] = useState('');

  const validation = validateServerUrl(value);

  const handleTest = async () => {
    if (!validation.valid) {
      setTestResult('✗ Invalid URL');
      return;
    }

    const start = Date.now();
    try {
      await apiService.getStatus();
      setTestResult(`✓ ${Date.now() - start}ms`);
    } catch {
      setTestResult('✗ Failed');
    }
  };

  return (
    <View>
      <ThemedText preset="labelMd" style={{ marginBottom: spacing.sm }}>
        Server URL
      </ThemedText>
      <TextInput
        value={value}
        onChangeText={onChange}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="http://192.168.1.100:8080"
        keyboardType="url"
        placeholderTextColor={colors.textSecondary}
        style={{
          borderWidth: 1,
          borderColor: validation.valid ? colors.border : colors.danger,
          borderRadius: 10,
          backgroundColor: colors.surface,
          color: colors.textPrimary,
          padding: spacing.sm,
          marginBottom: spacing.sm,
        }}
      />
      {!validation.valid && (
        <ThemedText preset="bodySm" style={{ color: colors.danger, marginBottom: spacing.sm }}>
          {validation.error}
        </ThemedText>
      )}

      <ThemedText preset="bodySm" style={{ color: colors.textSecondary, marginBottom: spacing.sm }}>
        {testResult || 'Ready to test connection'}
      </ThemedText>

      <View style={{ flexDirection: 'row', gap: spacing.sm }}>
        <Pressable
          onPress={handleTest}
          disabled={!validation.valid}
          style={{
            flex: 1,
            paddingVertical: 10,
            borderRadius: 8,
            backgroundColor: validation.valid ? colors.accentDim : colors.surfaceAlt,
            alignItems: 'center',
          }}
        >
          <ThemedText preset="labelMd" style={{ color: colors.textPrimary }}>
            Test Connection
          </ThemedText>
        </Pressable>
        <Pressable
          onPress={onSave}
          disabled={!validation.valid}
          style={{
            flex: 1,
            paddingVertical: 10,
            borderRadius: 8,
            backgroundColor: validation.valid ? colors.success : colors.surfaceAlt,
            alignItems: 'center',
          }}
        >
          <ThemedText preset="labelMd" style={{ color: colors.textPrimary }}>
            Save
          </ThemedText>
        </Pressable>
      </View>
    </View>
  );
};

import { PropsWithChildren, forwardRef } from 'react';
import { View, ViewProps } from 'react-native';
import { useTheme } from '../hooks/useTheme';

type ThemedViewProps = PropsWithChildren<ViewProps>;

export const ThemedView = forwardRef<View, ThemedViewProps>(({ children, style, ...props }, ref) => {
  const { colors } = useTheme();

  return (
    <View
      ref={ref}
      {...props}
      style={[
        {
          backgroundColor: colors.bg,
        },
        style,
      ]}
    >
      {children}
    </View>
  );
});

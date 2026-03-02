import { useContext } from 'react';
import { ThemeContext, ThemeContextValue } from '../theme/ThemeContext';

export const useTheme = (): ThemeContextValue => useContext(ThemeContext);

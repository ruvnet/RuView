import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { HeroUIProvider } from '@heroui/react';
import { StatCard } from '../components/common/StatCard';
import { GlowingBadge } from '../components/common/GlowingBadge';
import { CrystalLoader } from '../components/common/CrystalLoader';

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <HeroUIProvider>{children}</HeroUIProvider>
);

describe('StatCard', () => {
  it('renders title and value', () => {
    render(<StatCard title="Test Stat" value={1234} />, { wrapper });

    expect(screen.getByText('Test Stat')).toBeInTheDocument();
    expect(screen.getByText('1,234')).toBeInTheDocument();
  });

  it('renders string value correctly', () => {
    render(<StatCard title="String Stat" value="45.8 TFLOPS" />, { wrapper });

    expect(screen.getByText('45.8 TFLOPS')).toBeInTheDocument();
  });

  it('shows positive change indicator', () => {
    render(<StatCard title="Test" value={100} change={5.5} />, { wrapper });

    expect(screen.getByText(/5.5%/)).toBeInTheDocument();
    expect(screen.getByText(/↑/)).toBeInTheDocument();
  });

  it('shows negative change indicator', () => {
    render(<StatCard title="Test" value={100} change={-3.2} />, { wrapper });

    expect(screen.getByText(/3.2%/)).toBeInTheDocument();
    expect(screen.getByText(/↓/)).toBeInTheDocument();
  });

  it('applies different color variants', () => {
    const { rerender } = render(
      <StatCard title="Test" value={100} color="crystal" />,
      { wrapper }
    );

    expect(screen.getByText('Test')).toBeInTheDocument();

    rerender(
      <HeroUIProvider>
        <StatCard title="Test" value={100} color="temporal" />
      </HeroUIProvider>
    );

    expect(screen.getByText('Test')).toBeInTheDocument();
  });
});

describe('GlowingBadge', () => {
  it('renders children content', () => {
    render(<GlowingBadge>Test Badge</GlowingBadge>, { wrapper });

    expect(screen.getByText('Test Badge')).toBeInTheDocument();
  });

  it('applies different color variants', () => {
    render(<GlowingBadge color="success">Success</GlowingBadge>, { wrapper });

    expect(screen.getByText('Success')).toBeInTheDocument();
  });
});

describe('CrystalLoader', () => {
  it('renders without text', () => {
    const { container } = render(<CrystalLoader />, { wrapper });

    expect(container.firstChild).toBeInTheDocument();
  });

  it('renders with text', () => {
    render(<CrystalLoader text="Loading..." />, { wrapper });

    expect(screen.getByText('Loading...')).toBeInTheDocument();
  });

  it('supports different sizes', () => {
    const { rerender, container } = render(<CrystalLoader size="sm" />, { wrapper });
    expect(container.firstChild).toBeInTheDocument();

    rerender(<HeroUIProvider><CrystalLoader size="lg" /></HeroUIProvider>);
    expect(container.firstChild).toBeInTheDocument();
  });
});

import { type ReactNode } from "react";

export interface NavItem {
  id: string;
  label: string;
  icon: ReactNode;
}

interface SidebarProps {
  items: NavItem[];
  activeId: string;
  onNavigate: (id: string) => void;
}

// Minimal SVG icons to avoid external dependency
const ICONS: Record<string, ReactNode> = {
  dashboard: (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="7" height="9" rx="1" />
      <rect x="14" y="3" width="7" height="5" rx="1" />
      <rect x="14" y="12" width="7" height="9" rx="1" />
      <rect x="3" y="16" width="7" height="5" rx="1" />
    </svg>
  ),
  nodes: (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="5" r="3" />
      <circle cx="5" cy="19" r="3" />
      <circle cx="19" cy="19" r="3" />
      <line x1="12" y1="8" x2="5" y2="16" />
      <line x1="12" y1="8" x2="19" y2="16" />
    </svg>
  ),
  flash: (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
    </svg>
  ),
  server: (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="20" height="8" rx="2" />
      <rect x="2" y="14" width="20" height="8" rx="2" />
      <line x1="6" y1="6" x2="6.01" y2="6" />
      <line x1="6" y1="18" x2="6.01" y2="18" />
    </svg>
  ),
  settings: (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
    </svg>
  ),
};

export const DEFAULT_NAV_ITEMS: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: ICONS.dashboard },
  { id: "nodes", label: "Nodes", icon: ICONS.nodes },
  { id: "flash", label: "Flash", icon: ICONS.flash },
  { id: "server", label: "Server", icon: ICONS.server },
  { id: "settings", label: "Settings", icon: ICONS.settings },
];

export function Sidebar({ items, activeId, onNavigate }: SidebarProps) {
  return (
    <nav
      style={{
        width: "200px",
        minWidth: "200px",
        height: "100%",
        background: "var(--sidebar-bg, #12121a)",
        borderRight: "1px solid var(--border, #2e2e3e)",
        display: "flex",
        flexDirection: "column",
        padding: "16px 0",
      }}
    >
      {/* App title */}
      <div
        style={{
          padding: "0 20px 20px",
          fontSize: "18px",
          fontWeight: 800,
          color: "var(--text-primary, #e2e8f0)",
          letterSpacing: "-0.02em",
        }}
      >
        RuView
      </div>

      {/* Nav items */}
      <div style={{ display: "flex", flexDirection: "column", gap: "2px", flex: 1 }}>
        {items.map((item) => {
          const isActive = item.id === activeId;
          return (
            <button
              key={item.id}
              onClick={() => onNavigate(item.id)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                padding: "10px 20px",
                border: "none",
                background: isActive
                  ? "var(--accent-muted, rgba(99, 102, 241, 0.12))"
                  : "transparent",
                color: isActive
                  ? "var(--accent, #6366f1)"
                  : "var(--text-secondary, #94a3b8)",
                cursor: "pointer",
                fontSize: "13px",
                fontWeight: isActive ? 600 : 400,
                textAlign: "left",
                borderLeft: isActive
                  ? "3px solid var(--accent, #6366f1)"
                  : "3px solid transparent",
                transition: "background 0.1s, color 0.1s",
              }}
              onMouseEnter={(e) => {
                if (!isActive) {
                  e.currentTarget.style.background =
                    "var(--hover-bg, rgba(255,255,255,0.04))";
                  e.currentTarget.style.color = "var(--text-primary, #e2e8f0)";
                }
              }}
              onMouseLeave={(e) => {
                if (!isActive) {
                  e.currentTarget.style.background = "transparent";
                  e.currentTarget.style.color = "var(--text-secondary, #94a3b8)";
                }
              }}
            >
              {item.icon}
              {item.label}
            </button>
          );
        })}
      </div>

      {/* Version footer */}
      <div
        style={{
          padding: "12px 20px",
          fontSize: "10px",
          color: "var(--text-muted, #64748b)",
        }}
      >
        v0.3.0
      </div>
    </nav>
  );
}

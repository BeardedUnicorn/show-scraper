import { NavLink, Outlet } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import "./App.css";

const NAVIGATION = [
  { label: "Dashboard", to: "/" },
  { label: "Pending Posts", to: "/pending" },
  { label: "Venues", to: "/venues" },
  { label: "History", to: "/history" },
  { label: "Settings", to: "/settings" },
];

export default function App() {
  const navItems = useMemo(() => NAVIGATION, []);
  const [sidebarOpen, setSidebarOpen] = useState(() => {
    if (typeof window === "undefined") {
      return true;
    }
    return window.innerWidth >= 1024;
  });
  const [isDesktop, setIsDesktop] = useState(() => {
    if (typeof window === "undefined") {
      return true;
    }
    return window.innerWidth >= 1024;
  });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    function handleResize() {
      const desktop = window.innerWidth >= 1024;
      setIsDesktop(desktop);
      setSidebarOpen(desktop);
    }

    handleResize();
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  function handleToggleSidebar() {
    if (isDesktop) {
      return;
    }
    setSidebarOpen((prev) => !prev);
  }

  function handleCloseSidebar() {
    if (isDesktop) {
      return;
    }
    setSidebarOpen(false);
  }

  return (
    <div
      data-testid="app-shell"
      className={`app-shell${sidebarOpen ? " app-shell--sidebar-open" : ""}`}
    >
      <a data-testid="skip-to-content-link" className="skip-link" href="#main-content">
        Skip to content
      </a>
      {!isDesktop && sidebarOpen && (
        <button
          type="button"
          className="app-shell__backdrop"
          aria-label="Close navigation"
          onClick={handleCloseSidebar}
          data-testid="sidebar-backdrop"
        />
      )}
      <aside
        className="app-shell__sidebar"
        aria-label="Primary navigation"
        data-testid="sidebar"
      >
        <div className="app-shell__brand" data-testid="app-shell-brand">
          <span className="app-shell__logo" aria-hidden="true" data-testid="app-shell-logo">
            🎵
          </span>
          <div data-testid="app-shell-brand-text">
            <div className="app-shell__title" data-testid="app-shell-title">
              ShowScraper
            </div>
            <div className="app-shell__subtitle" data-testid="app-shell-subtitle">
              Tauri v2
            </div>
          </div>
        </div>
        <nav
          id="primary-navigation"
          className="app-shell__nav"
          data-testid="primary-navigation"
        >
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.to === "/"}
              data-testid={`nav-link-${
                item.to === "/" ? "dashboard" : item.to.replace(/\//g, "-")
              }`}
              className={({ isActive }) =>
                `app-shell__nav-link${isActive ? " app-shell__nav-link--active" : ""}`
              }
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
      </aside>
      <div className="app-shell__main" data-testid="app-shell-main">
        <header className="app-shell__header" data-testid="app-shell-header">
          <button
            type="button"
            className="app-shell__menu-button"
            onClick={handleToggleSidebar}
            aria-controls="primary-navigation"
            aria-expanded={sidebarOpen}
            data-testid="menu-button"
          >
            <span className="sr-only">Toggle navigation</span>
            ☰
          </button>
          <div data-testid="app-shell-header-text">
            <div className="app-shell__header-title" data-testid="app-shell-header-title">
              ShowScraper
            </div>
            <div
              className="app-shell__header-subtitle"
              data-testid="app-shell-header-subtitle"
            >
              Monitor scraping health, posting status, and upcoming tasks.
            </div>
          </div>
        </header>
        <main
          id="main-content"
          className="app-shell__content"
          tabIndex={-1}
          data-testid="main-content"
        >
          <Outlet />
        </main>
      </div>
    </div>
  );
}

export default function Dashboard() {
  return (
    <div className="card" data-testid="dashboard-card">
      <div className="section-header" data-testid="dashboard-header">
        <div className="section-header__title" data-testid="dashboard-title">
          Dashboard
        </div>
      </div>
      <p data-testid="dashboard-description">
        Monitor scraping health, posting status, and upcoming automation tasks here.
      </p>
    </div>
  );
}

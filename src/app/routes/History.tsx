export default function History() {
  return (
    <div className="card" data-testid="history-card">
      <div className="section-header" data-testid="history-header">
        <div className="section-header__title" data-testid="history-title">
          History
        </div>
      </div>
      <p data-testid="history-description">
        Review previously posted events once manual publishing is complete.
      </p>
    </div>
  );
}

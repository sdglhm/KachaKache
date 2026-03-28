function PrivacyBanner() {
  return (
    <footer className="mac-footer-pill">
      <span>Private & Offline • Runs 100% locally on your Mac</span>
      <span className="mac-footer-divider" aria-hidden>
        •
      </span>
      <a className="mac-footer-link" href="https://sdglhm.com" target="_blank" rel="noreferrer">
        sdglhm.com
      </a>
      <span className="mac-footer-divider" aria-hidden>
        •
      </span>
      <a
        className="mac-footer-link"
        href="https://github.com/sdglhm/KachaKache"
        target="_blank"
        rel="noreferrer"
      >
        GitHub
      </a>
    </footer>
  );
}

export default PrivacyBanner;

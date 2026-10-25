// Small inline SVG icons (24×24 grid, filled with currentColor).

const svg = (body: string) =>
  `<svg viewBox="0 0 24 24" aria-hidden="true" fill="currentColor">${body}</svg>`;

export const icons = {
  play: svg('<path d="M8 5.2c0-.8.9-1.3 1.6-.9l10 6.3c.6.4.6 1.4 0 1.8l-10 6.3c-.7.4-1.6-.1-1.6-.9z"/>'),
  pause: svg('<rect x="6" y="4.5" width="4.2" height="15" rx="1.3"/><rect x="13.8" y="4.5" width="4.2" height="15" rx="1.3"/>'),
  next: svg('<path d="M3.5 7c0-.8.9-1.3 1.6-.8l6.4 4.9c.5.4.5 1.3 0 1.7l-6.4 4.9c-.7.5-1.6 0-1.6-.8z"/><path d="M11.5 7c0-.8.9-1.3 1.6-.8l6.4 4.9c.5.4.5 1.3 0 1.7l-6.4 4.9c-.7.5-1.6 0-1.6-.8z"/>'),
  previous: svg('<path d="M20.5 7c0-.8-.9-1.3-1.6-.8l-6.4 4.9c-.5.4-.5 1.3 0 1.7l6.4 4.9c.7.5 1.6 0 1.6-.8z"/><path d="M12.5 7c0-.8-.9-1.3-1.6-.8l-6.4 4.9c-.5.4-.5 1.3 0 1.7l6.4 4.9c.7.5 1.6 0 1.6-.8z"/>'),
  expand: svg('<path d="M14 4h5a1 1 0 0 1 1 1v5h-2V7.4l-4.3 4.3-1.4-1.4L16.6 6H14zM4 14h2v2.6l4.3-4.3 1.4 1.4L7.4 18H10v2H5a1 1 0 0 1-1-1z"/>'),
  close: svg('<path d="M6.3 4.9 12 10.6l5.7-5.7 1.4 1.4-5.7 5.7 5.7 5.7-1.4 1.4-5.7-5.7-5.7 5.7-1.4-1.4 5.7-5.7-5.7-5.7z"/>'),
  chevronDown: svg('<path d="M5.3 8.3a1 1 0 0 1 1.4 0L12 13.6l5.3-5.3a1 1 0 1 1 1.4 1.4l-6 6a1 1 0 0 1-1.4 0l-6-6a1 1 0 0 1 0-1.4z"/>'),
  note: svg('<path d="M18 3.2v11.3a3.5 3.5 0 1 1-2-3.2V7.6l-7 1.6v7.3a3.5 3.5 0 1 1-2-3.2V6.4c0-.5.3-.9.8-1l9-2a1 1 0 0 1 1.2 1z"/>'),
};

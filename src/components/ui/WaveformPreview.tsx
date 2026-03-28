type WaveformPreviewProps = {
  samples: number[];
  calm?: boolean;
};

function WaveformPreview({ samples, calm }: WaveformPreviewProps) {
  const values = samples.length ? samples : Array.from({ length: 28 }, () => 0.05);
  const width = 320;
  const baseline = 24;

  const wavePath = values
    .map((value, index) => {
      const x = (index / (values.length - 1)) * width;
      const shaped = Math.pow(Math.max(0, Math.min(1, value)), 0.8);
      const amplitude = (calm ? 5 : 11) + shaped * (calm ? 3 : 14);
      const y = baseline - amplitude;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" L ");

  return (
    <div className="mac-wave-line">
      <svg viewBox={`0 0 ${width} 48`} className="h-full w-full" preserveAspectRatio="none" aria-hidden>
        <path d={`M 0 ${baseline} L ${width} ${baseline}`} stroke="rgba(118,124,137,0.35)" strokeWidth="1" fill="none" />
        <path
          d={`M ${wavePath}`}
          stroke="url(#waveGradient)"
          strokeWidth="2"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <defs>
          <linearGradient id="waveGradient" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0%" stopColor="#5d87ff" />
            <stop offset="100%" stopColor="#6f63ec" />
          </linearGradient>
        </defs>
      </svg>
    </div>
  );
}

export default WaveformPreview;

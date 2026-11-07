export function LightField() {
  return (
    <>
      <div className="field" aria-hidden="true">
        <span className="a" />
        <span className="b" />
        <span className="c" />
      </div>
      <svg
        className="caustics"
        aria-hidden="true"
        width="100%"
        height="100%"
        preserveAspectRatio="none"
      >
        <filter id="caustic-noise">
          <feTurbulence
            type="fractalNoise"
            baseFrequency="0.008 0.012"
            numOctaves={3}
            seed={7}
          />
          <feColorMatrix values="0 0 0 0 1  0 0 0 0 0.95  0 0 0 0 0.8  0 0 0 2.2 -0.9" />
        </filter>
        <rect width="100%" height="100%" filter="url(#caustic-noise)" />
      </svg>
    </>
  );
}

export function PhoneDemo() {
  return (
    <>
      <figure
        className="phone-demo"
        aria-label="Preview of the reader with the agent panel"
      >
        <div className="sheet-page">
          <h4>12. Term and Termination</h4>
          <p>
            12.1 Either party may terminate this Lease on the third anniversary
            of the Commencement Date by giving the other party{" "}
            <mark>not less than six months' written notice</mark>.
          </p>
          <span className="trace" aria-hidden="true" />
        </div>
        <div className="agent-sheet">
          <div className="grab" aria-hidden="true" />
          <div className="head">
            <b>Claude Code</b>
            <span>
              <i />
              Ready
            </span>
          </div>
          <p>
            Either side can leave at year three with six months' written notice.{" "}
            <span className="cite">p. 4, 12.1</span>
          </p>
        </div>
      </figure>
      <p className="phone-demo-tag">
        <i />
        Agent panel preview, in development
      </p>
    </>
  );
}

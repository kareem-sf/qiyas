/** A determinate progress bar on Base UI's Progress: a label, the percentage at the other end (Latin
 *  digits, as the window shows every number), and a thin track that fills. Use it where the work
 *  reports how far it is (a PDF page being drawn, an update downloading); a wait with no percentage
 *  keeps the `DotLoader`. The role, `aria-valuenow` and the label are Base UI's. */
import { Progress as BaseProgress } from "@base-ui/react/progress";

type Props = {
  /** 0 to 100. */
  value: number;
  /** What is being done ("Drawing the page…"). */
  label: string;
  /** Without the label's line: for the status bar, where the text beside says it. */
  compact?: boolean;
  className?: string;
};

export function Progress({ value, label, compact = false, className = "" }: Props) {
  const shown = Math.max(0, Math.min(100, Math.round(value)));
  return (
    <BaseProgress.Root value={shown} className={`progress${compact ? " compact" : ""} ${className}`.trim()}>
      <div className="progress-head">
        <BaseProgress.Label className="progress-label">{label}</BaseProgress.Label>
        <BaseProgress.Value className="progress-value">{() => `${shown} %`}</BaseProgress.Value>
      </div>
      <BaseProgress.Track className="progress-track">
        <BaseProgress.Indicator className="progress-fill" />
      </BaseProgress.Track>
    </BaseProgress.Root>
  );
}

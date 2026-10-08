import { useT } from "../i18n";

type Props = {
  cursor: [number, number] | null;
  units: string;
  unitsWarning: boolean;
  problems: number;
  message: string;
};

/** Bottom line: what is happening, cursor position in drawing units, units and file warnings. */
export function StatusBar({ cursor, units, unitsWarning, problems, message }: Props) {
  const t = useT();
  return (
    <footer className="status">
      <span className="message" role="status" aria-live="polite">
        {message}
      </span>
      {problems > 0 && <span className="warn">{t("recovered", { n: problems })}</span>}
      {unitsWarning && <span className="warn">{t("unitsUnknown")}</span>}
      {units && <span>{units}</span>}
      {/* Coordinates read left to right in both languages. */}
      <span className="cursor" dir="ltr">
        {cursor ? `${cursor[0].toFixed(3)}, ${cursor[1].toFixed(3)}` : ""}
      </span>
    </footer>
  );
}

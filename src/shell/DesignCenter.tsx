import { useEffect, useRef, useState } from "react";
import { useT } from "../i18n";

type Props = {
  /** Asks for another drawing (the Open dialog); null when none was chosen. */
  choose(): Promise<string | null>;
  /** That drawing's named blocks, read-only. */
  list(path: string): Promise<string[]>;
  /** The open drawing has a block of this name. */
  has(name: string): boolean;
  /** Starts placing the block, its definition copied from `path` when this drawing lacks it. */
  onInsert(name: string, path: string): void;
  /** This drawing's block of that name takes the other drawing's contents. */
  onRedefine(name: string, path: string): Promise<void>;
  onClose(): void;
};

/** Another drawing's blocks, opened read-only: insert one (its definition is copied), or
 *  redefine this drawing's block of the same name from it. */
export function DesignCenter({ choose, list, has, onInsert, onRedefine, onClose }: Props) {
  const t = useT();
  const dialog = useRef<HTMLDialogElement>(null);
  const [path, setPath] = useState<string | null>(null);
  const [names, setNames] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.current?.showModal();
    return () => opener?.focus();
  }, []);

  async function act(work: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await work();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const open = () =>
    act(async () => {
      const chosen = await choose();
      if (!chosen) return;
      setNames(await list(chosen));
      setPath(chosen);
    });

  return (
    <dialog
      ref={dialog}
      className="settings design-center"
      aria-labelledby="design-center-title"
      onClose={onClose}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <h2 id="design-center-title" className="heading">
        {t("block.centerTitle")}
      </h2>
      <p>
        <button type="button" disabled={busy} onClick={open}>
          {t("block.openDrawing")}
        </button>{" "}
        {path && <span dir="auto">{path}</span>}
      </p>
      {path && names.length === 0 && <p>{t("block.none")}</p>}
      <ul className="design-center-blocks">
        {path &&
          names.map((name) => (
            <li key={name}>
              <span dir="auto">{name}</span>
              <button
                type="button"
                aria-label={t("block.insertOne", { name })}
                onClick={() => {
                  dialog.current?.close();
                  onInsert(name, path);
                }}
              >
                {t("block.insert")}
              </button>
              {has(name) && (
                <button
                  type="button"
                  aria-label={t("block.redefineOne", { name })}
                  disabled={busy}
                  onClick={() => act(() => onRedefine(name, path))}
                >
                  {t("block.redefine")}
                </button>
              )}
            </li>
          ))}
      </ul>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="buttons">
        <button type="button" onClick={() => dialog.current?.close()}>
          {t("block.close")}
        </button>
      </div>
    </dialog>
  );
}

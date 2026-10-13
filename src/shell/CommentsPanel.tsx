import { memo } from "react";
import type { CommentInfo, Sheet } from "../api";
import { useT } from "../i18n";

type Props = {
  /** Null while not read yet. */
  comments: CommentInfo[] | null;
  sheets: Sheet[];
  /** Reads the comments again. Missing while no drawing is open. */
  onRefresh?(): void;
  /** Shows a comment's cloud on the drawing. */
  onShow(comment: CommentInfo): void;
};

/** The comment clouds of every sheet (COMMENT, or Mizan's add_comment), each with its sheet; a
 *  click shows its cloud. Memoised: the app re-renders on every pointer move. */
export const CommentsPanel = memo(function CommentsPanel({ comments, sheets, onRefresh, onShow }: Props) {
  const t = useT();
  const sheetName = (block: string) =>
    sheets.find((s) => s.block.toLowerCase() === block.toLowerCase())?.name ?? block;
  return (
    <aside className="conflicts comments">
      <div className="panel-head conflicts-head">
        <h2 className="heading">{t("comments")}</h2>
        <button type="button" disabled={!onRefresh} onClick={onRefresh}>
          {t("comments.refresh")}
        </button>
      </div>
      {comments && comments.length === 0 && <p className="empty">{t("comments.empty")}</p>}
      {comments && comments.length > 0 && (
        <ul>
          {comments.map((c, i) => (
            <li key={`${c.handle}`} className="conflict">
              <button type="button" className="what link" title={t("comments.show")} onClick={() => onShow(c)}>
                <p className="reason" dir="auto">
                  {i + 1}. {c.text}
                </p>
                <p className="meta">
                  <span className="sheets" dir="auto">
                    {sheetName(c.sheet)}
                  </span>
                </p>
              </button>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
});

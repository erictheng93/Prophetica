"use client";
import { useEffect, useState } from "react";
import type { KycDocument } from "@/lib/member-management-shared";

const KIND_LABEL: Record<KycDocument["kind"], string> = {
  id_doc: "證件",
  selfie: "自拍",
  liveness: "Liveness",
};

export function KycDocumentGallery({ userId }: { userId: string }) {
  const [docs, setDocs] = useState<KycDocument[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [failedDocIds, setFailedDocIds] = useState<ReadonlySet<string>>(
    new Set(),
  );

  useEffect(() => {
    let active = true;
    setFailedDocIds(new Set());
    fetch(`/api/members/${encodeURIComponent(userId)}/kyc-documents`, {
      cache: "no-store",
    })
      .then(async (r) => {
        if (!r.ok) throw new Error("load_failed");
        return (await r.json()) as KycDocument[];
      })
      .then((d) => {
        if (active) setDocs(d);
      })
      .catch(() => {
        if (active) setError("文件暫時無法載入。");
      });
    return () => {
      active = false;
    };
  }, [userId]);

  if (error)
    return <p className="text-[14px] text-proph-ink-muted-80">{error}</p>;
  if (!docs)
    return <p className="text-[14px] text-proph-ink-muted-80">載入中…</p>;
  if (docs.length === 0)
    return <p className="text-[14px] text-proph-ink-muted-80">尚無文件。</p>;

  return (
    <div className="grid grid-cols-2 gap-3">
      {docs.map((doc) => (
        <figure key={doc.doc_id} className="space-y-1">
          {failedDocIds.has(doc.doc_id) ? (
            <p className="text-[13px] text-proph-ink-muted-80">
              文件暫時無法載入。
            </p>
          ) : (
            <img
              src={`/api/members/${encodeURIComponent(userId)}/kyc-documents/${encodeURIComponent(doc.doc_id)}`}
              alt={KIND_LABEL[doc.kind]}
              className="w-full rounded-proph-sm border border-proph-hairline"
              onError={() => {
                setFailedDocIds((prev) => new Set(prev).add(doc.doc_id));
              }}
            />
          )}
          <figcaption className="text-[12px] text-proph-ink-muted-80">
            {KIND_LABEL[doc.kind]} · {doc.captured_at}
          </figcaption>
        </figure>
      ))}
    </div>
  );
}

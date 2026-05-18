import { redirect } from "next/navigation";
import { AdminShell } from "@/components/admin-shell";
import { FaqManagementForm } from "@/components/faq-management-form";
import { adminCopy } from "@/lib/admin-copy";
import { loadFaqCatalogForViewer } from "@/lib/faq-admin";
import { getFaqCatalogStore } from "@/lib/faq-store";
import { getViewer } from "@/lib/session";

export default async function AdminFaqPage() {
  const viewer = await getViewer();
  if (!viewer) {
    redirect("/login");
  }

  const view = await loadFaqCatalogForViewer({
    viewer,
    store: getFaqCatalogStore(),
  });

  return (
    <AdminShell
      title={adminCopy.faq.title}
      description={adminCopy.faq.description}
      eyebrow={adminCopy.faq.eyebrow}
    >
      <FaqManagementForm
        initialCatalog={view.catalog}
        initialSource={view.source}
        canEdit={view.canEdit}
      />
    </AdminShell>
  );
}

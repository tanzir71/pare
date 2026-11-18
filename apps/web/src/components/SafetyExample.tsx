import { FileRow } from "@pare/ui";
import { Archive, File, Folder, GitBranch } from "lucide-react";

const GB = 1e9;

/**
 * "What Pare decided": four example rows (Protected, Review, Review, Safe) on a light card
 * inside the dark safety band. Purely illustrative, so the page hides it from assistive tech
 * and gives a text summary instead.
 */
export function SafetyExample() {
  return (
    <div className="overflow-hidden rounded-xl bg-bg-surface">
      <div className="bg-bg-subtle px-5 py-3.5 text-body-m-strong text-text-primary">
        What Pare decided
      </div>
      <FileRow
        icon={GitBranch}
        hue="slate"
        name="node_modules · client-portal"
        path="D:\code\client-portal"
        badge={{ tone: "protected", label: "Uncommitted" }}
        age="Changed 3 days ago"
        size={1.1 * GB}
        disabled
      />
      <FileRow
        icon={File}
        hue="blue"
        name="Q3-report-final.pdf"
        path="C:\Users\ana\Downloads"
        badge={{ tone: "review", label: "Review" }}
        age="Opened 8 months ago"
        size={48e6}
      />
      <FileRow
        icon={Archive}
        hue="violet"
        name="Codex archived sessions"
        path="C:\Users\ana\.codex\archived_sessions"
        badge={{ tone: "review", label: "Review" }}
        age="Oldest Jan 2026"
        size={1.6 * GB}
      />
      <FileRow
        icon={Folder}
        hue="teal"
        name="npm download cache"
        path="C:\Users\ana\AppData\Local\npm-cache"
        badge={{ tone: "safe", label: "Rebuilds" }}
        age="Recreated as needed"
        size={2.4 * GB}
        checked
        className="border-b-0"
      />
    </div>
  );
}

export function Terminal({
  html,
  chrome = true,
}: {
  html: string;
  chrome?: boolean;
}) {
  return (
    <div className="overflow-hidden rounded-md border border-ds-gray-300 bg-ds-bg-300">
      {chrome && (
        <div className="flex items-center gap-1.5 border-b border-ds-gray-300 px-4 py-2.5">
          <div className="h-2.5 w-2.5 rounded-full bg-[#ff5f57]" />
          <div className="h-2.5 w-2.5 rounded-full bg-[#febc2e]" />
          <div className="h-2.5 w-2.5 rounded-full bg-[#28c840]" />
        </div>
      )}
      <div
        className="shiki-scope overflow-x-auto"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}

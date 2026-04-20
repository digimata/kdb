import Link from "next/link";

import { PlusIcon } from "@/components/ui/icons";
import { cn } from "@/lib/utils";

const navColumns: { href: string; label: string }[][] = [
  [
    { href: "https://github.com/dremnik/kdb", label: "GitHub" },
    {
      href: "https://github.com/dremnik/kdb/blob/master/CHANGELOG.md",
      label: "Changelog",
    },
    { href: "https://github.com/dremnik/kdb/issues", label: "Issues" },
  ],
  [
    {
      href: "https://github.com/dremnik/kdb/blob/master/LICENSE",
      label: "MIT",
    },
  ],
];

function CornerPlus({ className }: { className?: string }) {
  return (
    <PlusIcon
      aria-hidden
      className={cn("size-3 text-[#73798C]", className)}
    />
  );
}

function DigimataMark() {
  return (
    <span
      className="select-none text-[24px] leading-none text-ds-gray-1000"
      aria-hidden
    >
      Ξ
    </span>
  );
}

export function Footer() {
  const year = new Date().getFullYear();
  return (
    <footer className="w-full px-6 pt-10 pb-24">
      <div className="relative mx-auto flex max-w-md flex-col gap-32 px-8 py-14 sm:max-w-lg md:max-w-2xl md:gap-40 md:px-10 md:py-16 lg:max-w-3xl lg:px-12">
        <CornerPlus className="absolute top-0 left-0" />
        <CornerPlus className="absolute top-0 right-0" />
        <CornerPlus className="absolute bottom-0 left-0" />
        <CornerPlus className="absolute bottom-0 right-0" />

        {/* Top row: brand + tagline | nav */}
        <div className="flex flex-col gap-12 sm:flex-row sm:justify-between sm:gap-0">
          <div className="flex flex-col gap-4">
            <Link href="/">
              <span
                className="font-mono text-ds-gray-1000"
                style={{
                  fontWeight: 300,
                  fontSize: "24px",
                  lineHeight: "normal",
                  letterSpacing: "-0.04em",
                }}
              >
                kdb
              </span>
            </Link>
            <p className="text-copy-14 max-w-xs text-ds-gray-1000">
              A knowledge database for your project.
            </p>
          </div>

          <div className="flex gap-10 sm:gap-14">
            {navColumns.map((column, i) => (
              <nav key={i} className="flex flex-col items-start gap-4">
                {column.map((link) => {
                  const isExternal = link.href.startsWith("http");
                  const Tag = isExternal ? "a" : Link;
                  const extra = isExternal
                    ? { target: "_blank" as const, rel: "noopener noreferrer" }
                    : {};
                  return (
                    <Tag
                      key={link.label}
                      href={link.href}
                      className="group relative pb-1 text-label-13 text-ds-gray-600 transition-colors hover:text-ds-gray-1000"
                      {...extra}
                    >
                      {link.label}
                      <span className="absolute bottom-0 left-0 h-px w-full bg-ds-gray-100" />
                      <span className="absolute bottom-0 left-0 h-px w-full origin-left scale-x-0 bg-ds-steel-500 transition-transform duration-300 ease-out group-hover:scale-x-100" />
                    </Tag>
                  );
                })}
              </nav>
            ))}
          </div>
        </div>

        {/* Bottom row: legal | mark */}
        <div className="flex items-end justify-between">
          <p className="text-label-13 text-ds-gray-1000">
            Digital Automata, Inc.
            <span className="ml-2 text-ds-gray-600">© {year}</span>
          </p>
          <DigimataMark />
        </div>
      </div>
    </footer>
  );
}

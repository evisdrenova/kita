import React, { memo, useState } from "react";
import { Section } from "./types/types";
import { Layers2, ChevronDown, ChevronUp } from "lucide-react";
import { Separator } from "./components/ui/separator";

interface SectionNavProps {
  sections: Section[];
  activeSection: number | null;
  setActiveSection: (val: number | null) => void;
  totalCount?: number;
  searchQuery?: string;
}

const PaginatedSection = ({
  section,
  searchQuery,
}: {
  section: Section;
  searchQuery?: string;
}) => {
  const [showMore, setShowMore] = useState(false);

  const shouldPaginate = !searchQuery?.trim();

  const limitedComponent =
    shouldPaginate && !showMore
      ? section.getLimitedComponent?.(5)
      : section.component;

  return (
    <div className="flex flex-col gap-1">
      {limitedComponent}
      {shouldPaginate && (section.counts || 0) > 5 && (
        <button
          onClick={() => setShowMore((prev) => !prev)}
          className="text-xs  text-gray-400 hover:text-gray-200 flex items-center gap-1 py-1 self-start cursor-pointer pl-2"
        >
          {!showMore ? (
            <ChevronDown className="w-3 h-3" />
          ) : (
            <ChevronUp className="w-3 h-3" />
          )}
          {!showMore ? `Show ${(section.counts || 0) - 5} more` : `Show less`}
        </button>
      )}
    </div>
  );
};

const SectionNav = (props: SectionNavProps) => {
  const { sections, activeSection, setActiveSection, totalCount, searchQuery } =
    props;

  // If activeSection is null, we want to show "All" content
  const activeComponent =
    activeSection !== null
      ? sections.find((s) => s.id === activeSection)?.component
      : null;

  return (
    <div className="flex flex-col">
      <nav className="flex flex-row gap-2 border-b border-b-border sticky top-0 dark:bg-zinc-800">
        <div className="px-3 py-2 flex gap-2 overflow-auto scrollbar-none">
          {/* All button*/}
          <NavButton
            key="all"
            section={{
              id: -1,
              name: "All",
              icon: <Layers2 className="w-4 h-4" />,
              counts: totalCount,
            }}
            isActive={activeSection === null}
            onClick={() => setActiveSection(null)}
          />
          {sections.map((section) => (
            <NavButton
              key={section.id}
              section={section}
              isActive={activeSection === section.id}
              onClick={() => setActiveSection(section.id)}
            />
          ))}
        </div>
      </nav>

      <div className="p-2">
        {/* When "All" is selected (activeSection is null), render all components with pagination */}
        {activeSection === null ? (
          <div className="flex flex-col gap-4">
            {sections
              .filter((sect) => sect.counts && sect.counts > 0)
              .map((section, index) => (
                <div key={section.id}>
                  <h2 className="text-sm font-medium text-gray-800 dark:text-gray-200 mb-2">
                    {section.name}
                    {section.counts !== undefined && (
                      <span className="ml-2 text-gray-800 dark:text-gray-500">
                        ({section.counts})
                      </span>
                    )}
                  </h2>
                  <PaginatedSection
                    section={section}
                    searchQuery={searchQuery}
                  />
                  {index != sections.length - 1 && <Separator />}
                </div>
              ))}
          </div>
        ) : (
          activeComponent
        )}
      </div>
    </div>
  );
};

const NavButton = memo(
  ({
    section,
    isActive,
    onClick,
  }: {
    section: {
      id: number;
      name: string;
      icon: React.ReactNode;
      counts?: number;
    };
    isActive: boolean;
    onClick: () => void;
  }) => (
    <button
      onClick={onClick}
      className={`
      flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium
      transition-colors duration-150 cursor-pointer
      ${
        isActive
          ? "bg-blue-500/20 text-gray-200 dark:text-white-400 ring-1 ring-blue-400/30"
          : "text-gray-400 hover:bg-white/5"
      }
    `}
    >
      {section.icon}
      {section.name}
      {section.counts !== undefined && section.counts > 0 && (
        <span
          className={`
          inline-flex items-center justify-center rounded-full px-1.5 
          ${
            isActive
              ? "bg-blue-400/20 text-blue-800 dark:text-blue-200"
              : "bg-white/5 text-gray-800 dark:text-gray-500"
          }
        `}
        >
          {section.counts}
        </span>
      )}
    </button>
  )
);

export default memo(SectionNav);

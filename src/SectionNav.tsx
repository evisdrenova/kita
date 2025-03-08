import React, { memo } from "react";
import { Section } from "./types/types";
import { Layers2 } from "lucide-react";

interface SectionNavProps {
  sections: Section[];
  activeSection: number | null;
  setActiveSection: (val: number | null) => void;
}

const SectionNav = (props: SectionNavProps) => {
  const { sections, activeSection, setActiveSection } = props;

  // If activeSection is null, we want to show "All" content
  const activeComponent =
    activeSection !== null
      ? sections.find((s) => s.id === activeSection)?.component
      : null;

  return (
    <div className="flex flex-col">
      <nav className="flex flex-row gap-2 border-b border-b-border">
        <div className="px-3 py-2 flex gap-2 overflow-x-auto scrollbar-none">
          {/* Special "All" button */}
          <NavButton
            key="all"
            section={{
              id: -1,
              name: "All",
              icon: <Layers2 className="w-4 h-4" />,
            }}
            isActive={activeSection === null}
            onClick={() => setActiveSection(null)}
          />

          {/* Regular section buttons */}
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
        {/* When "All" is selected (activeSection is null), render all components */}
        {activeSection === null ? (
          <div className="flex flex-col gap-4">
            {sections.map((section) => (
              <div key={section.id}>
                <h2 className="text-sm font-medium text-gray-400 mb-2">
                  {section.name}
                </h2>
                {section.component}
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
      transition-colors duration-150
      ${
        isActive
          ? "bg-blue-500/20 text-white-400 ring-1 ring-blue-400/30"
          : "text-gray-400 hover:bg-white/5"
      }
    `}
    >
      {section.icon}
      {section.name}
      {section.counts && (
        <span
          className={`
          inline-flex items-center justify-center rounded-full px-1.5 
          ${
            isActive ? "bg-red-400/20 text-red-400" : "bg-white/5 text-gray-500"
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

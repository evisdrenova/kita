import React, { useState } from "react";
import TitleBar, { searchCategories, SearchCategory } from "../Titlebar";
import { Input } from "../../components/ui/input";

export default function Home() {
  const [selectedCategories, setSelectedCategories] = useState<
    Set<SearchCategory>
  >(new Set(searchCategories));

  const toggleCategory = (category: SearchCategory) => {
    setSelectedCategories((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(category)) {
        newSet.delete(category);
      } else {
        newSet.add(category);
      }
      return newSet;
    });
  };

  const results = [
    { id: 1, title: "Result 1" },
    { id: 2, title: "Result 2" },
    { id: 3, title: "Result 3" },
  ];

  return (
    <div className="flex flex-col h-full">
      <TitleBar
        selectedCategories={selectedCategories}
        toggleCategory={toggleCategory}
      />
      <Input />
      <SearchResults results={results} />
    </div>
  );
}

interface SearchResults {
  id: number;
  title: string;
}

interface SearchResultsProps {
  results: SearchResults[];
}

function SearchResults(props: SearchResultsProps) {
  const { results } = props;

  return (
    <div>
      {results.map((res) => (
        <div key={res.id}>{res.title}</div>
      ))}
    </div>
  );
}

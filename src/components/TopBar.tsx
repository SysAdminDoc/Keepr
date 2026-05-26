import { Menu, Search, RefreshCw, Settings, Moon, Sun, X } from "lucide-react";
import { useStore } from "../store";

interface Props {
  onMenu: () => void;
}

export function TopBar({ onMenu }: Props) {
  const { search, setSearch, dark, toggleDark, openSettings, load } = useStore();
  return (
    <header className="sticky top-0 z-30 flex items-center h-16 px-2 bg-white dark:bg-[#202124] border-b border-gray-200 dark:border-[#5f6368]">
      <button
        className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
        onClick={onMenu}
        title="Menu"
      >
        <Menu size={20} />
      </button>
      <div className="flex items-center gap-2 px-2 mr-4 select-none">
        <div className="w-8 h-8 rounded-md bg-[#FBBC04] grid place-items-center text-[#202124] font-bold">
          K
        </div>
        <span className="text-[22px] text-gray-700 dark:text-gray-200 font-product hidden sm:inline">
          Keepr
        </span>
      </div>
      <div className="flex-1 max-w-2xl mx-auto">
        <div className="flex items-center bg-[#f1f3f4] dark:bg-[#3c4043] rounded-lg px-3 h-12 focus-within:bg-white dark:focus-within:bg-[#202124] focus-within:shadow-md">
          <Search size={20} className="text-gray-500 dark:text-gray-400" />
          <input
            type="text"
            placeholder="Search"
            className="flex-1 bg-transparent outline-none px-3 text-base"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          {search && (
            <button
              onClick={() => setSearch("")}
              className="p-1 rounded-full hover:bg-gray-300 dark:hover:bg-[#5f6368]"
            >
              <X size={18} />
            </button>
          )}
        </div>
      </div>
      <div className="flex items-center pl-2">
        <button
          className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
          onClick={() => load()}
          title="Refresh"
        >
          <RefreshCw size={20} />
        </button>
        <button
          className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
          onClick={toggleDark}
          title={dark ? "Light mode" : "Dark mode"}
        >
          {dark ? <Sun size={20} /> : <Moon size={20} />}
        </button>
        <button
          className="p-3 rounded-full hover:bg-gray-200 dark:hover:bg-[#3c4043]"
          onClick={openSettings}
          title="Settings"
        >
          <Settings size={20} />
        </button>
      </div>
    </header>
  );
}

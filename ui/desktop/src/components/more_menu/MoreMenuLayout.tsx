import { useState } from 'react';
import MoreMenu from './MoreMenu';
import type { View, ViewOptions } from '../../App';
import { Document } from '../icons';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../ui/Tooltip';
import { ChevronRight } from 'lucide-react';

export default function MoreMenuLayout({
  hasMessages,
  showMenu = true,
  setView,
  setIsGoosehintsModalOpen,
  isSidePanelVisible,
  toggleSidePanel,
}: {
  hasMessages?: boolean;
  showMenu?: boolean;
  setView?: (view: View, viewOptions?: ViewOptions) => void;
  setIsGoosehintsModalOpen?: (isOpen: boolean) => void;
  isSidePanelVisible?: boolean;
  toggleSidePanel?: () => void;
}) {
  const [isTooltipOpen, setIsTooltipOpen] = useState(false);

  // Assume macOS if not explicitly set
  const safeIsMacOS = (window?.electron?.platform || 'darwin') === 'darwin';

  return (
    <div
      className="relative flex items-center h-14 border-b border-borderSubtle w-full"
      style={{ WebkitAppRegion: 'drag' }}
    >
      {showMenu && (
        <div
          className={`flex items-center justify-between w-full h-full ${safeIsMacOS ? 'pl-[86px]' : 'pl-[8px]'} pr-4`}
        >
          <TooltipProvider>
            <div className="flex items-center gap-2">
              {/* Side panel toggle */}
              {!isSidePanelVisible && toggleSidePanel && (
                <button
                  aria-label="Show sidebar"
                  className="no-drag hover:cursor-pointer border border-borderSubtle hover:border-borderStandard rounded-lg p-2 h-9 w-9 text-textSubtle hover:text-textStandard flex items-center justify-center transition-colors bg-bgSecondary/80 dark:bg-bgAppInverse/80"
                  onClick={toggleSidePanel}
                >
                  {/* Using lucide-react icon */}
                  <ChevronRight className="h-4 w-4 text-textStandard dark:text-textStandard" />
                </button>
              )}

              <Tooltip open={isTooltipOpen} onOpenChange={setIsTooltipOpen}>
                <TooltipTrigger asChild>
                  <button
                    className="z-[100] no-drag hover:cursor-pointer border border-borderSubtle hover:border-borderStandard rounded-lg p-2 pr-3 text-textSubtle hover:text-textStandard text-sm flex items-center transition-colors [&>svg]:size-4 "
                    onClick={async () => {
                      if (hasMessages) {
                        window.electron.directoryChooser();
                      } else {
                        window.electron.directoryChooser(true);
                      }
                    }}
                  >
                    <Document className="mr-1" />
                    <div className="max-w-[200px] truncate [direction:rtl]">
                      {String(window.appConfig.get('GOOSE_WORKING_DIR'))}
                    </div>
                  </button>
                </TooltipTrigger>
                <TooltipContent className="max-w-96 overflow-auto scrollbar-thin" side="top">
                  {window.appConfig.get('GOOSE_WORKING_DIR') as string}
                </TooltipContent>
              </Tooltip>
            </div>
          </TooltipProvider>

          <MoreMenu
            setView={setView || (() => {})}
            setIsGoosehintsModalOpen={setIsGoosehintsModalOpen || (() => {})}
          />
        </div>
      )}
    </div>
  );
}

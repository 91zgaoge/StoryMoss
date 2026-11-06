import React, { useState } from 'react';
import { ChevronDown } from 'lucide-react';

interface PanelProps {
  title: string;
  children: React.ReactNode;
  collapsible?: boolean;
  defaultOpen?: boolean;
}

const headerClassName = [
  'flex items-center justify-between px-4 py-3',
  'border-b border-borderSubtle',
].join(' ');

export const Panel: React.FC<PanelProps> = ({
  title,
  children,
  collapsible,
  defaultOpen = true,
}) => {
  const [open, setOpen] = useState(defaultOpen);
  const contentId = React.useId();

  const headerContent = (
    <>
      <h3 className="font-body text-[13px] font-semibold tracking-wide text-ai-ink-2">{title}</h3>
      {collapsible && (
        <ChevronDown
          aria-hidden="true"
          className={[
            'w-4 h-4 text-cinema-gold/80 transition-transform duration-500 ease-spring motion-reduce:duration-[0.01s] motion-reduce:ease-linear',
            open ? 'rotate-180' : '',
          ].join(' ')}
        />
      )}
    </>
  );

  return (
    <div className="rounded-panel border border-borderSubtle bg-cinema-900/40 p-1 shadow-panel">
      <div
        className="overflow-hidden rounded-[calc(var(--radius-md)-4px)] bg-cinema-850"
        style={{
          boxShadow: 'inset 0 1px 0 color-mix(in oklch, white 6%, transparent)',
        }}
      >
        {collapsible ? (
          <button
            type="button"
            aria-expanded={open}
            aria-controls={contentId}
            className={[
              headerClassName,
              'w-full text-left bg-transparent border-0 cursor-pointer hover:bg-cinema-800/50',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50',
            ].join(' ')}
            onClick={() => setOpen(v => !v)}
          >
            {headerContent}
          </button>
        ) : (
          <div className={headerClassName}>{headerContent}</div>
        )}
        <div
          id={contentId}
          className={[
            'transition-opacity duration-500 ease-spring motion-reduce:duration-[0.01s] motion-reduce:ease-linear',
            collapsible ? 'overflow-hidden' : '',
            collapsible ? (open ? 'max-h-none opacity-100' : 'max-h-0 opacity-0') : 'opacity-100',
          ].join(' ')}
        >
          <div className="p-4">{children}</div>
        </div>
      </div>
    </div>
  );
};

interface SectionTitleProps {
  label?: string;
  title: string;
  description?: string;
  align?: 'center' | 'left';
}

export function SectionTitle({ label, title, description, align = 'center' }: SectionTitleProps) {
  const alignClass = align === 'center' ? 'text-center' : 'text-left';

  return (
    <div className={`${alignClass} mb-16 md:mb-20`}>
      {label && (
        <span className="mb-3 inline-block font-mono text-xs uppercase tracking-widest text-charcoal">
          {label}
        </span>
      )}
      <h2 className="mb-4 text-[28px] leading-tight tracking-[-0.015em] text-ink md:text-[40px]">
        {title}
      </h2>
      {description && (
        <p className="mx-auto max-w-[560px] text-base leading-relaxed text-charcoal md:text-lg">
          {description}
        </p>
      )}
    </div>
  );
}

import type {
  ReactNode,
  ButtonHTMLAttributes,
  AnchorHTMLAttributes,
} from "react";

type CommonProps = {
  variant: "primary" | "secondary";
  children: ReactNode;
  className?: string;
};

type ButtonProps = CommonProps &
  ButtonHTMLAttributes<HTMLButtonElement> & { as?: "button" };
type AnchorProps = CommonProps &
  AnchorHTMLAttributes<HTMLAnchorElement> & { as: "a" };

export type InkButtonProps = ButtonProps | AnchorProps;

export function InkButton(props: InkButtonProps) {
  const base =
    "inline-flex items-center justify-center rounded-full px-6 py-3 text-sm font-medium transition-[transform,background-color,color] duration-200 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-moss focus-visible:ring-offset-2 focus-visible:ring-offset-canvas";
  const styles =
    props.variant === "primary"
      ? "bg-moss text-canvas [@media(hover:hover)]:hover:bg-moss-soft"
      : "surface-2 border border-subtle text-paper hover:surface-3";

  if (props.as === "a" || ("href" in props && props.href)) {
    const {
      as,
      variant,
      children,
      className = "",
      ...rest
    } = props as AnchorProps;
    return (
      <a className={`${base} ${styles} ${className}`} {...rest}>
        {children}
      </a>
    );
  }

  const {
    as,
    variant,
    children,
    className = "",
    ...rest
  } = props as ButtonProps;
  return (
    <button className={`${base} ${styles} ${className}`} {...rest}>
      {children}
    </button>
  );
}

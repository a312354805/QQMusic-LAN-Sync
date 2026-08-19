import type { ButtonHTMLAttributes, ReactNode } from 'react'
import clsx from 'clsx'

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string
  children: ReactNode
  tone?: 'default' | 'primary'
  size?: 'sm' | 'md' | 'lg'
}

export function IconButton({ label, children, className, tone = 'default', size = 'md', ...props }: Props) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={clsx('icon-button', `icon-button--${tone}`, `icon-button--${size}`, className)}
      {...props}
    >
      {children}
    </button>
  )
}

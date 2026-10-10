import { useEffect, useState } from 'react'
import { useDebounce } from '../hooks/useDebounce'

const DEBOUNCE_MS = 300

interface SearchBoxProps {
  label: string
  /** The committed (URL) value. */
  value: string
  onSearch: (value: string) => void
}

/** Text input that reports changes via `onSearch` 300 ms after typing stops. */
export function SearchBox({ label, value, onSearch }: SearchBoxProps) {
  const [text, setText] = useState(value)
  const debounced = useDebounce(text, DEBOUNCE_MS)

  // Follow external changes (back/forward navigation).
  useEffect(() => setText(value), [value])

  useEffect(() => {
    if (debounced !== value) onSearch(debounced)
    // Only a settled keystroke should trigger a search, not a changed `value`.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [debounced])

  return (
    <input
      type="search"
      aria-label={label}
      placeholder={label}
      value={text}
      onChange={(event) => setText(event.target.value)}
    />
  )
}

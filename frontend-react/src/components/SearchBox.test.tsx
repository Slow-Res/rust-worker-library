import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SearchBox } from './SearchBox'

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('SearchBox', () => {
  it('reports the value only after 300 ms without typing', () => {
    const onSearch = vi.fn()
    render(<SearchBox label="Search" value="" onSearch={onSearch} />)
    const input = screen.getByRole('searchbox', { name: 'Search' })

    fireEvent.change(input, { target: { value: 'du' } })
    act(() => void vi.advanceTimersByTime(200))
    fireEvent.change(input, { target: { value: 'dune' } })
    act(() => void vi.advanceTimersByTime(299))
    expect(onSearch).not.toHaveBeenCalled()

    act(() => void vi.advanceTimersByTime(1))
    expect(onSearch).toHaveBeenCalledExactlyOnceWith('dune')
  })

  it('does not search on mount', () => {
    const onSearch = vi.fn()
    render(<SearchBox label="Search" value="abc" onSearch={onSearch} />)
    act(() => void vi.advanceTimersByTime(1000))
    expect(onSearch).not.toHaveBeenCalled()
  })

  it('follows the committed value (e.g. back navigation) without re-searching', () => {
    const onSearch = vi.fn()
    const { rerender } = render(<SearchBox label="Search" value="one" onSearch={onSearch} />)

    rerender(<SearchBox label="Search" value="two" onSearch={onSearch} />)
    act(() => void vi.advanceTimersByTime(1000))

    expect(screen.getByRole('searchbox')).toHaveValue('two')
    expect(onSearch).not.toHaveBeenCalled()
  })
})

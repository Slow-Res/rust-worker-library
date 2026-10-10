import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { NewLoanPage } from './NewLoanPage'

afterEach(() => vi.unstubAllGlobals())

const book = {
  id: 7,
  title: 'Dune',
  author: 'Frank Herbert',
  isbn: null,
  published_year: null,
  total_copies: 2,
  available_copies: 2,
  created_at: '',
  updated_at: '',
}
const member = { id: 3, name: 'Ada', email: 'ada@example.com', active_loans: 0, created_at: '', updated_at: '' }

const page = (items: unknown[]) => ({ items, page: 1, per_page: 20, total: items.length })

/** Books match only the empty search; any search text returns nothing. */
function stubApi() {
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string) => {
      const params = new URL(url, 'http://x').searchParams
      if (url.startsWith('/api/books')) return Promise.resolve(Response.json(page(params.get('q') ? [] : [book])))
      return Promise.resolve(Response.json(page([member])))
    }),
  )
}

function renderPage() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={['/loans/new']}>
        <NewLoanPage />
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

async function waitForOption(name: string, label: RegExp) {
  const select = await screen.findByRole('combobox', { name })
  await screen.findByRole('option', { name: label })
  return select
}
const bookSelect = () => waitForOption('Book', /Dune/)
const memberSelect = () => waitForOption('Member', /Ada/)

describe('NewLoanPage pickers', () => {
  it('disables submit until both a book and a member are chosen', async () => {
    stubApi()
    renderPage()
    const submit = screen.getByRole('button', { name: 'Lend book' })
    expect(submit).toBeDisabled()

    await userEvent.selectOptions(await bookSelect(), '7')
    expect(submit).toBeDisabled()
    await userEvent.selectOptions(await memberSelect(), '3')
    expect(submit).toBeEnabled()
  })

  it('keeps the selected book when the search no longer matches it', async () => {
    stubApi()
    renderPage()
    const select = await bookSelect()
    await userEvent.selectOptions(select, '7')

    await userEvent.type(screen.getByRole('searchbox', { name: 'Search books' }), 'zzz')

    // Wait for the (debounced) empty search result to arrive.
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledWith(expect.stringContaining('q=zzz'), expect.anything()))
    expect(select).toHaveValue('7')
    expect(screen.getByRole('option', { name: /Dune/ })).toBeInTheDocument()
  })
})

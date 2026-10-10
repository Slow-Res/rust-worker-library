import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate, useParams } from 'react-router'
import { ApiRequestError } from '../api/client'
import { bookKey, createBook, getBook, updateBook } from '../api/books'
import { BookForm } from '../components/BookForm'
import { ErrorMessage } from '../components/ErrorMessage'
import type { BookInput } from '../generated/BookInput'
import { parsePositiveInt } from '../utils'
import { NotFoundPage } from './NotFoundPage'

/** `/books/new` (no id param) and `/books/:id/edit`. */
export function BookFormPage() {
  const { id: rawId } = useParams()
  if (rawId === undefined) return <BookFormView />
  const id = parsePositiveInt(rawId)
  return id === null ? <NotFoundPage /> : <BookFormView id={id} />
}

function BookFormView({ id }: { id?: number }) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const existing = useQuery({
    queryKey: bookKey(id ?? 0),
    queryFn: () => getBook(id ?? 0),
    enabled: id !== undefined,
  })

  const save = useMutation({
    mutationFn: (input: BookInput) => (id === undefined ? createBook(input) : updateBook(id, input)),
    onSuccess: async () => {
      await Promise.all(['books', 'book', 'loans'].map((key) => queryClient.invalidateQueries({ queryKey: [key] })))
      void navigate('/books')
    },
  })

  if (id !== undefined) {
    if (existing.isPending) return <p aria-busy="true">Loading…</p>
    if (existing.error) {
      const notFound = existing.error instanceof ApiRequestError && existing.error.status === 404
      return notFound ? <NotFoundPage /> : <ErrorMessage error={existing.error} />
    }
  }

  return (
    <>
      <h1>{id === undefined ? 'Add book' : 'Edit book'}</h1>
      <BookForm
        initial={existing.data}
        error={save.error}
        isSubmitting={save.isPending}
        onSubmit={(input) => save.mutate(input)}
      />
    </>
  )
}

import { useMutation, useQueryClient } from '@tanstack/react-query'
import { returnLoan } from '../api/loans'

/** Returning a loan changes availability and member loan counts too. */
export function useReturnLoan() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: returnLoan,
    onSuccess: async () => {
      await Promise.all(
        ['loans', 'books', 'book', 'members', 'member'].map((key) => queryClient.invalidateQueries({ queryKey: [key] })),
      )
    },
  })
}

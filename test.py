def get_amount_out(amount_in, reserve_in, reserve_out):
    """
    Calculate the amount out for a Uniswap V2 swap.
    
    :param amount_in: The amount of tokens being sold
    :param reserve_in: The reserve of the token being sold in the pool
    :param reserve_out: The reserve of the token being bought in the pool
    :return: The amount of tokens that will be received
    """
    assert amount_in > 0, "Amount in must be positive"
    assert reserve_in > 0 and reserve_out > 0, "Reserves must be positive"
    
    # Calculate the amount in with fee applied (0.3% fee)
    amount_in_with_fee = amount_in * 997
    
    # Calculate numerator and denominator
    numerator = amount_in_with_fee * reserve_out
    denominator = (reserve_in * 1000) + amount_in_with_fee
    
    # Calculate amount out
    amount_out = numerator // denominator
    
    return amount_out

# Example usage
#amount_in = 1e18  # 1 million tokens
#reserve_in = 14827541321296988190592  # 5 billion tokens
#reserve_out =  52004797525850  # 2.5 million tokens

#Found profitable path:
#0xa478c2975ab1ea89e8196811f51a7b7ade33eb11 (7534856512359425429745486, 2154830442785117309384) 348607678442569574868 -> 
#0xe928a5619257da0336f292bd10decdbabd4cd9b6 (2787013733903140673136, 18339432715015) 2033476966797 -> 
#0x559f1117143324629fa0580012cf2444b219537c (2809895677702791189448, 8956451075384) 518645846138489857162 -> 
#0xfd0a40bc83c5fae4203dec7e5929b446b07d1c76 (8093669447489852655550, 2314085254173546191) 138964530663267816 -> 


amount_in = 1e17  # 1 million tokens
reserve_in =  350647400207015589546 # 5 billion tokens
reserve_out =  3833216667147255   # 2.5 million tokens



amount_out = get_amount_out(amount_in, reserve_in, reserve_out)
print(f"Amount out: {amount_out }")
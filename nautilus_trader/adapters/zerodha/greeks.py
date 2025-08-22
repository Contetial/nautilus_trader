# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
#  https://nautechsystems.io
#
#  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
#  You may not use this file except in compliance with the License.
#  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
#
#  Unless required by applicable law or agreed to in writing, software
#  distributed under the License is distributed on an "AS IS" BASIS,
#  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#  See the License for the specific language governing permissions and
#  limitations under the License.
# -------------------------------------------------------------------------------------------------

"""
Greeks calculation utilities for options trading.

This module provides Black-Scholes based Greeks calculations optimized
for Indian options markets with support for NSE/BSE instruments.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import NamedTuple

from nautilus_trader.model.enums import OptionKind
from nautilus_trader.model.instruments import Option


class Greeks(NamedTuple):
    """
    Options Greeks container.
    
    Attributes
    ----------
    delta : float
        Price sensitivity to underlying price changes.
    gamma : float
        Rate of change of delta.
    theta : float
        Time decay (per day).
    vega : float
        Sensitivity to volatility changes.
    rho : float
        Sensitivity to interest rate changes.
    """
    delta: float
    gamma: float
    theta: float
    vega: float
    rho: float


@dataclass
class OptionPricingInputs:
    """
    Inputs for options pricing calculations.
    
    Attributes
    ----------
    spot_price : float
        Current underlying price.
    strike_price : float
        Option strike price.
    time_to_expiry : float
        Time to expiry in years.
    risk_free_rate : float
        Risk-free interest rate (annualized).
    volatility : float
        Implied volatility (annualized).
    dividend_yield : float
        Dividend yield (annualized).
    option_kind : OptionKind
        CALL or PUT.
    """
    spot_price: float
    strike_price: float
    time_to_expiry: float
    risk_free_rate: float
    volatility: float
    dividend_yield: float
    option_kind: OptionKind


class GreeksCalculator:
    """
    Black-Scholes Greeks calculator for options.
    
    This calculator provides accurate Greeks calculations using the
    Black-Scholes-Merton model, with adjustments for Indian market conventions.
    """
    
    # Indian market risk-free rate (approximate RBI repo rate)
    DEFAULT_RISK_FREE_RATE = 0.065  # 6.5%
    DEFAULT_DIVIDEND_YIELD = 0.015   # 1.5% average dividend yield
    
    @staticmethod
    def calculate_greeks(inputs: OptionPricingInputs) -> Greeks:
        """
        Calculate all Greeks for an option.
        
        Parameters
        ----------
        inputs : OptionPricingInputs
            The pricing inputs.
            
        Returns
        -------
        Greeks
            The calculated Greeks.
        """
        if inputs.time_to_expiry <= 0:
            # Expired option
            return Greeks(0.0, 0.0, 0.0, 0.0, 0.0)
            
        if inputs.volatility <= 0:
            # Zero volatility - intrinsic value only
            return GreeksCalculator._intrinsic_greeks(inputs)
        
        # Calculate d1 and d2
        d1, d2 = GreeksCalculator._calculate_d1_d2(inputs)
        
        # Standard normal CDF and PDF
        n_d1 = GreeksCalculator._norm_cdf(d1)
        n_d2 = GreeksCalculator._norm_cdf(d2)
        n_prime_d1 = GreeksCalculator._norm_pdf(d1)
        
        # Discount factors
        exp_neg_rt = math.exp(-inputs.risk_free_rate * inputs.time_to_expiry)
        exp_neg_qt = math.exp(-inputs.dividend_yield * inputs.time_to_expiry)
        
        # Calculate Greeks based on option type
        if inputs.option_kind == OptionKind.CALL:
            delta = exp_neg_qt * n_d1
            rho = inputs.strike_price * inputs.time_to_expiry * exp_neg_rt * n_d2
        else:  # PUT
            delta = exp_neg_qt * (n_d1 - 1)
            rho = -inputs.strike_price * inputs.time_to_expiry * exp_neg_rt * (1 - n_d2)
        
        # Gamma (same for calls and puts)
        gamma = (exp_neg_qt * n_prime_d1) / (
            inputs.spot_price * inputs.volatility * math.sqrt(inputs.time_to_expiry)
        )
        
        # Theta (time decay per day)
        theta_term1 = -(inputs.spot_price * n_prime_d1 * inputs.volatility * exp_neg_qt) / (
            2 * math.sqrt(inputs.time_to_expiry)
        )
        theta_term2 = inputs.risk_free_rate * inputs.strike_price * exp_neg_rt
        theta_term3 = inputs.dividend_yield * inputs.spot_price * exp_neg_qt
        
        if inputs.option_kind == OptionKind.CALL:
            theta = (theta_term1 - theta_term2 * n_d2 + theta_term3 * n_d1) / 365
        else:  # PUT
            theta = (theta_term1 + theta_term2 * (1 - n_d2) - theta_term3 * (1 - n_d1)) / 365
        
        # Vega (sensitivity to volatility)
        vega = inputs.spot_price * exp_neg_qt * n_prime_d1 * math.sqrt(inputs.time_to_expiry) / 100
        
        return Greeks(
            delta=delta,
            gamma=gamma,
            theta=theta,
            vega=vega,
            rho=rho / 100  # Rho per 1% change in interest rate
        )
    
    @staticmethod
    def calculate_option_price(inputs: OptionPricingInputs) -> float:
        """
        Calculate theoretical option price using Black-Scholes.
        
        Parameters
        ----------
        inputs : OptionPricingInputs
            The pricing inputs.
            
        Returns
        -------
        float
            The theoretical option price.
        """
        if inputs.time_to_expiry <= 0:
            # Expired option - return intrinsic value
            if inputs.option_kind == OptionKind.CALL:
                return max(0, inputs.spot_price - inputs.strike_price)
            else:
                return max(0, inputs.strike_price - inputs.spot_price)
        
        if inputs.volatility <= 0:
            # Zero volatility - return intrinsic value
            if inputs.option_kind == OptionKind.CALL:
                return max(0, inputs.spot_price - inputs.strike_price)
            else:
                return max(0, inputs.strike_price - inputs.spot_price)
        
        # Calculate d1 and d2
        d1, d2 = GreeksCalculator._calculate_d1_d2(inputs)
        
        # Standard normal CDF
        n_d1 = GreeksCalculator._norm_cdf(d1)
        n_d2 = GreeksCalculator._norm_cdf(d2)
        
        # Discount factors
        exp_neg_rt = math.exp(-inputs.risk_free_rate * inputs.time_to_expiry)
        exp_neg_qt = math.exp(-inputs.dividend_yield * inputs.time_to_expiry)
        
        if inputs.option_kind == OptionKind.CALL:
            price = (inputs.spot_price * exp_neg_qt * n_d1 - 
                    inputs.strike_price * exp_neg_rt * n_d2)
        else:  # PUT
            price = (inputs.strike_price * exp_neg_rt * (1 - n_d2) - 
                    inputs.spot_price * exp_neg_qt * (1 - n_d1))
        
        return max(0, price)
    
    @staticmethod
    def calculate_implied_volatility(
        market_price: float,
        inputs: OptionPricingInputs,
        max_iterations: int = 100,
        tolerance: float = 1e-6
    ) -> float | None:
        """
        Calculate implied volatility using Newton-Raphson method.
        
        Parameters
        ----------
        market_price : float
            The observed market price.
        inputs : OptionPricingInputs
            The pricing inputs (volatility will be ignored).
        max_iterations : int
            Maximum number of iterations.
        tolerance : float
            Convergence tolerance.
            
        Returns
        -------
        float | None
            The implied volatility, or None if calculation failed.
        """
        if inputs.time_to_expiry <= 0 or market_price <= 0:
            return None
        
        # Initial guess
        vol = 0.25  # 25% initial volatility guess
        
        for _ in range(max_iterations):
            # Create inputs with current volatility guess
            test_inputs = OptionPricingInputs(
                spot_price=inputs.spot_price,
                strike_price=inputs.strike_price,
                time_to_expiry=inputs.time_to_expiry,
                risk_free_rate=inputs.risk_free_rate,
                volatility=vol,
                dividend_yield=inputs.dividend_yield,
                option_kind=inputs.option_kind
            )
            
            # Calculate price and vega at current volatility
            price = GreeksCalculator.calculate_option_price(test_inputs)
            greeks = GreeksCalculator.calculate_greeks(test_inputs)
            
            # Price difference
            price_diff = price - market_price
            
            # Check convergence
            if abs(price_diff) < tolerance:
                return vol
            
            # Newton-Raphson update: vol = vol - f(vol)/f'(vol)
            # f(vol) = theoretical_price - market_price
            # f'(vol) = vega * 100 (since vega is per 1% vol change)
            if abs(greeks.vega) < 1e-10:  # Avoid division by zero
                break
                
            vol = vol - price_diff / (greeks.vega * 100)
            
            # Keep volatility in reasonable bounds
            vol = max(0.001, min(5.0, vol))  # 0.1% to 500%
        
        return None  # Failed to converge
    
    @staticmethod
    def _calculate_d1_d2(inputs: OptionPricingInputs) -> tuple[float, float]:
        """Calculate d1 and d2 for Black-Scholes formula."""
        sqrt_t = math.sqrt(inputs.time_to_expiry)
        
        d1 = (math.log(inputs.spot_price / inputs.strike_price) + 
              (inputs.risk_free_rate - inputs.dividend_yield + 
               0.5 * inputs.volatility**2) * inputs.time_to_expiry) / (
              inputs.volatility * sqrt_t)
        
        d2 = d1 - inputs.volatility * sqrt_t
        
        return d1, d2
    
    @staticmethod
    def _norm_cdf(x: float) -> float:
        """Standard normal cumulative distribution function."""
        return 0.5 * (1 + math.erf(x / math.sqrt(2)))
    
    @staticmethod
    def _norm_pdf(x: float) -> float:
        """Standard normal probability density function."""
        return math.exp(-0.5 * x**2) / math.sqrt(2 * math.pi)
    
    @staticmethod
    def _intrinsic_greeks(inputs: OptionPricingInputs) -> Greeks:
        """Calculate Greeks for intrinsic value (zero volatility case)."""
        if inputs.option_kind == OptionKind.CALL:
            delta = 1.0 if inputs.spot_price > inputs.strike_price else 0.0
        else:  # PUT
            delta = -1.0 if inputs.spot_price < inputs.strike_price else 0.0
        
        return Greeks(
            delta=delta,
            gamma=0.0,
            theta=0.0,
            vega=0.0,
            rho=0.0
        )
    
    @staticmethod
    def time_to_expiry_years(expiry_datetime: datetime) -> float:
        """
        Calculate time to expiry in years.
        
        Parameters
        ----------
        expiry_datetime : datetime
            The expiry datetime (assumed to be 3:30 PM IST).
            
        Returns
        -------
        float
            Time to expiry in years.
        """
        now = datetime.now(timezone.utc)
        
        # Convert expiry to UTC if needed
        if expiry_datetime.tzinfo is None:
            # Assume IST (UTC+5:30) if no timezone info
            import pytz
            ist = pytz.timezone('Asia/Kolkata')
            expiry_datetime = ist.localize(expiry_datetime).astimezone(timezone.utc)
        
        # Calculate time difference
        time_diff = expiry_datetime - now
        
        # Convert to years (using 365.25 days per year)
        return max(0, time_diff.total_seconds() / (365.25 * 24 * 3600))


class OptionsAnalytics:
    """
    Advanced options analytics for strategy development.
    
    This class provides higher-level analytics functions for options
    trading strategies in Indian markets.
    """
    
    @staticmethod
    def calculate_breakeven_points(
        strategy_legs: list[tuple[Option, int, float]]  # (option, quantity, entry_price)
    ) -> list[float]:
        """
        Calculate breakeven points for an options strategy.
        
        Parameters
        ----------
        strategy_legs : list[tuple[Option, int, float]]
            List of (option_instrument, quantity, entry_price) tuples.
            Positive quantity = long, negative = short.
            
        Returns
        -------
        list[float]
            List of breakeven spot prices.
        """
        # This would implement complex breakeven calculations
        # For now, return empty list as placeholder
        return []
    
    @staticmethod
    def calculate_max_profit_loss(
        strategy_legs: list[tuple[Option, int, float]]
    ) -> tuple[float, float]:
        """
        Calculate maximum profit and loss for a strategy.
        
        Parameters
        ----------
        strategy_legs : list[tuple[Option, int, float]]
            Strategy legs.
            
        Returns
        -------
        tuple[float, float]
            (max_profit, max_loss). Infinite values represented as float('inf').
        """
        # Placeholder implementation
        return (float('inf'), float('-inf'))
    
    @staticmethod
    def calculate_portfolio_greeks(
        positions: list[tuple[Option, int, Greeks]]  # (option, quantity, greeks)
    ) -> Greeks:
        """
        Calculate portfolio-level Greeks.
        
        Parameters
        ----------
        positions : list[tuple[Option, int, Greeks]]
            List of (option, quantity, individual_greeks) tuples.
            
        Returns
        -------
        Greeks
            Portfolio Greeks.
        """
        total_delta = 0.0
        total_gamma = 0.0
        total_theta = 0.0
        total_vega = 0.0
        total_rho = 0.0
        
        for option, quantity, greeks in positions:
            # Multiply by quantity and lot size
            multiplier = quantity * option.lot_size
            
            total_delta += greeks.delta * multiplier
            total_gamma += greeks.gamma * multiplier
            total_theta += greeks.theta * multiplier
            total_vega += greeks.vega * multiplier
            total_rho += greeks.rho * multiplier
        
        return Greeks(
            delta=total_delta,
            gamma=total_gamma,
            theta=total_theta,
            vega=total_vega,
            rho=total_rho
        )